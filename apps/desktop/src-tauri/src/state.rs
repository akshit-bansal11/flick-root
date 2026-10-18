//! Shared application state, startup tasks and shutdown.
use crate::error::CommandError;
use crate::icons::Icons;
use crate::meta_store::JsonMeta;
use anyhow::Context;
use arc_swap::ArcSwap;
use features::{EventSink, FeatureCtx, Registry, TaskScope, builtin_features};
use index_apps::{AppEntry, AppIndex, cache};
use index_files::{FileIndex, Overlay, Root, WatchHandle, default_roots, scan, snapshot};
use launcher_core::config::{Config, config_path, local_data_dir};
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use launcher_core::store::Store;
use launcher_core::{Input, Item};
use parking_lot::Mutex;
use providers::KeySource;
use providers::hub::ProviderHub;
use providers::secret::{KeyringBackend, SecretVault, VaultKeys};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const APP_REFRESH: Duration = Duration::from_secs(600);

/// Apps and the fuzzy index built over their names; swapped together.
pub struct AppMatch {
    /// Entries, same order as the index.
    pub entries: Arc<Vec<AppEntry>>,
    /// Fuzzy index over entry names.
    pub index: MatchIndex,
}

/// Everything commands and tasks share.
pub struct AppState {
    /// Live config.
    pub config: Arc<ArcSwap<Config>>,
    /// SQLite store.
    pub store: Arc<Store>,
    /// Installed apps.
    pub apps: Arc<AppIndex>,
    /// Apps plus fuzzy index.
    pub app_match: ArcSwap<AppMatch>,
    /// Reused matcher scratch.
    pub matcher: Mutex<FuzzyMatcher>,
    /// File index.
    pub files: Arc<FileIndex>,
    /// Live file changes.
    pub overlay: ArcSwap<Overlay>,
    /// File watcher, held while Find is on.
    pub watch: Mutex<Option<WatchHandle>>,
    /// Write-only secret vault (Credential Manager plus metadata).
    pub vault: Arc<SecretVault>,
    /// Stops a running scan on exit.
    pub scan_cancel: Arc<AtomicBool>,
    /// Feature registry.
    pub registry: Arc<Registry>,
    /// Handles given to features.
    pub ctx: FeatureCtx,
    /// App icons.
    pub icons: Icons,
    /// Items of the latest search, by id.
    pub last_results: Mutex<HashMap<String, Item>>,
    /// Input of the latest search.
    pub last_input: Mutex<Option<Input>>,
    /// Latest search sequence number.
    pub latest_seq: AtomicU64,
    /// Key source over the vault, for building providers.
    pub keys: Arc<dyn KeySource>,
    config_path: PathBuf,
    config_write: Mutex<()>,
    cache_dir: PathBuf,
}

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        if let Err(err) = self.0.emit(name, payload) {
            tracing::warn!(%err, name, "emit failed");
        }
    }
}

/// Builds the fuzzy index for `entries`.
pub fn build_match(entries: Arc<Vec<AppEntry>>) -> AppMatch {
    let names: Vec<&str> = entries.iter().map(|a| a.name.as_str()).collect();
    let index = MatchIndex::new(&names);
    AppMatch { entries, index }
}

impl AppState {
    /// Applies `f` to a copy of the config, saves it atomically, publishes it and
    /// rebuilds the provider hub. Writers are serialized; `f` errors change nothing.
    pub fn update_config<R>(
        &self,
        f: impl FnOnce(&mut Config) -> Result<R, CommandError>,
    ) -> Result<R, CommandError> {
        let _guard = self.config_write.lock();
        let mut next = (*self.config.load_full()).clone();
        let out = f(&mut next)?;
        next.save(&self.config_path)
            .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
        self.config.store(Arc::new(next));
        self.rebuild_hub();
        Ok(out)
    }

    /// Rebuilds the provider hub from the live config.
    pub fn rebuild_hub(&self) {
        let hub = ProviderHub::from_config(&self.config.load().providers, &self.keys);
        self.ctx.providers.store(Arc::new(hub));
    }

    fn rebuild_match(&self) {
        self.app_match
            .store(Arc::new(build_match(self.apps.entries())));
    }

    fn snapshot_path(&self) -> PathBuf {
        self.cache_dir.join("files.idx")
    }

    fn save_snapshot(&self) {
        if self.files.is_empty() {
            return;
        }
        if let Err(err) = snapshot::save(&self.snapshot_path(), &self.files.load()) {
            tracing::warn!(%err, "saving file snapshot failed");
        }
    }
}

fn roots_for(cfg: &Config) -> Vec<Root> {
    let find = &cfg.modes.find;
    let mut roots: Vec<Root> = default_roots()
        .into_iter()
        .filter(|r| find.roots.iter().any(|n| n.eq_ignore_ascii_case(&r.label)))
        .collect();
    for p in &find.extra_roots {
        let path = PathBuf::from(p);
        if let Ok(id) = u8::try_from(roots.len())
            && path.is_dir()
        {
            let label = path
                .file_name()
                .map_or_else(|| p.clone(), |n| n.to_string_lossy().into_owned());
            roots.push(Root { id, path, label });
        }
    }
    roots
}

/// Creates and manages [`AppState`]. Cheap: no scanning here.
pub fn init(app: &tauri::App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let config_path = config_path().context("locating config")?;
    let config = Arc::new(ArcSwap::from_pointee(
        Config::load(&config_path).unwrap_or_else(|err| {
            tracing::warn!(%err, "config unreadable, using defaults");
            Config::default()
        }),
    ));
    let data = local_data_dir().context("locating data dir")?;
    std::fs::create_dir_all(&data).context("creating data dir")?;
    let (store, report) = Store::open(&data.join("store.db")).context("opening store")?;
    let store = Arc::new(store);
    if report.reset
        && let Err(err) = handle.emit("store://reset", serde_json::json!({}))
    {
        tracing::warn!(%err, "emit failed");
    }
    let cache_dir = cache::cache_dir().context("locating cache dir")?;
    std::fs::create_dir_all(&cache_dir).context("creating cache dir")?;
    let apps = Arc::new(AppIndex::new());
    if let Err(err) = apps.load_cache(&cache_dir) {
        tracing::warn!(%err, "app cache unreadable");
    }
    let vault = Arc::new(SecretVault::new(
        Arc::new(KeyringBackend),
        Arc::new(JsonMeta::open(data.join("secret_meta.json"))),
    ));
    let keys: Arc<dyn KeySource> = Arc::new(VaultKeys(vault.clone()));
    let hub = ProviderHub::from_config(&config.load().providers, &keys);
    let ctx = FeatureCtx {
        config: config.clone(),
        store: store.clone(),
        providers: Arc::new(ArcSwap::from_pointee(hub)),
        http: reqwest::Client::builder()
            .build()
            .context("building http client")?,
        events: Arc::new(TauriSink(handle)),
        tasks: TaskScope::new(),
    };
    let state = Arc::new(AppState {
        config,
        store,
        app_match: ArcSwap::from_pointee(build_match(apps.entries())),
        apps,
        matcher: Mutex::new(FuzzyMatcher::new()),
        files: Arc::new(FileIndex::new()),
        overlay: ArcSwap::from_pointee(Overlay::new()),
        watch: Mutex::new(None),
        vault,
        scan_cancel: Arc::new(AtomicBool::new(false)),
        registry: Arc::new(Registry::new()),
        ctx,
        icons: Icons::new(index_apps::icons::IconCache::new(&cache_dir)),
        last_results: Mutex::new(HashMap::new()),
        last_input: Mutex::new(None),
        latest_seq: AtomicU64::new(0),
        keys,
        config_path,
        config_write: Mutex::new(()),
        cache_dir,
    });
    app.manage(state);
    Ok(())
}

/// Spawns background startup work; returns immediately.
pub fn start(app: &AppHandle) {
    let state: Arc<AppState> = Arc::clone(&*app.state::<Arc<AppState>>());
    let s = state.clone();
    tauri::async_runtime::spawn(async move {
        for f in builtin_features() {
            s.registry.register(f).await;
        }
        let cfg = s.config.load_full();
        s.registry.apply_config(&cfg, &s.ctx).await;
    });
    let s = state.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(APP_REFRESH);
        loop {
            tick.tick().await;
            refresh_apps(&s).await;
        }
    });
    if state.config.load().modes.find.enabled {
        tauri::async_runtime::spawn(async move { start_find(&state).await });
    }
}

pub(crate) async fn refresh_apps(s: &Arc<AppState>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let stats = st.apps.refresh_blocking()?;
        st.apps.save_cache(&st.cache_dir)?;
        st.rebuild_match();
        // Icons after the list is usable; ensure skips files that exist.
        let _com = index_apps::com::ComGuard::new().map_err(index_apps::AppsError::Enumerate)?;
        for e in st.apps.entries().iter() {
            if let Err(err) = st.icons.cache().ensure(e) {
                tracing::debug!(%err, app = %e.name, "icon extraction failed");
            }
        }
        Ok::<_, index_apps::AppsError>(stats)
    })
    .await;
    match res {
        Ok(Ok(stats)) => tracing::info!(kept = stats.kept, "apps refreshed"),
        Ok(Err(err)) => tracing::warn!(%err, "app refresh failed"),
        Err(err) => tracing::warn!(%err, "app refresh task failed"),
    }
}

/// Loads the snapshot, scans the roots and starts the watcher (Find on, or a reindex).
pub(crate) async fn start_find(s: &Arc<AppState>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        match snapshot::load(&st.snapshot_path()) {
            Ok(Some(store)) => st.files.replace(store),
            Ok(None) => {}
            Err(err) => tracing::warn!(%err, "file snapshot unreadable"),
        }
        let roots = roots_for(&st.config.load());
        let store = scan(&roots, &st.scan_cancel, |_| {})?;
        st.files.replace(store);
        st.save_snapshot();
        let handle = st.files.watch_auto(&roots)?;
        st.overlay.store(handle.overlay());
        *st.watch.lock() = Some(handle);
        Ok::<_, index_files::FilesError>(())
    })
    .await;
    match res {
        Ok(Ok(())) => tracing::info!("find index ready"),
        Ok(Err(err)) => tracing::warn!(%err, "find index failed"),
        Err(err) => tracing::warn!(%err, "find task failed"),
    }
}

/// Find off: drops the watcher and frees the file index memory.
pub(crate) fn stop_find(s: &AppState) {
    s.watch.lock().take();
    s.overlay.store(Arc::new(Overlay::new()));
    s.files
        .replace(index_files::store::StoreBuilder::new().finish(Vec::new()));
}

/// Stops features, flushes the store and saves the file snapshot.
pub fn shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    state.scan_cancel.store(true, Ordering::Relaxed);
    state.watch.lock().take();
    tauri::async_runtime::block_on(state.registry.stop_all());
    state.store.shutdown();
    state.save_snapshot();
}
