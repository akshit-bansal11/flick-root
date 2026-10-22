//! Shared application state, startup tasks and shutdown.
use crate::error::CommandError;
use crate::icons::Icons;
use crate::meta_store::JsonMeta;
use anyhow::Context;
use arc_swap::ArcSwap;
use features::media::{Media, spotify::Spotify};
use features::{EventSink, FeatureCtx, Registry, TaskScope, builtin_features_with};
use index_apps::{AppEntry, AppIndex, cache};
use index_files::drives;
use index_files::programs::unix_now;
use index_files::{
    FileIndex, FilesError, Overlay, ProgramCache, ProgramFile, Root, WatchHandle, default_roots,
    scan, snapshot,
};
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
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use win::drives::DriveKind;

/// Apps (Start menu, registry, desktop) are refreshed on open when older than this (seconds).
const APPS_MAX_AGE: u64 = 600;
/// Programs get a full rescan on open when the last full scan is older than this (seconds).
const PROGRAMS_MAX_AGE: u64 = 24 * 3600;
/// The first open of the process reconciles Find when the snapshot is older than this (seconds).
const FIND_MAX_AGE: u64 = 600;

static START: OnceLock<Instant> = OnceLock::new();

/// Records the process start; call first thing in `run`. Later calls are no-ops.
pub fn mark_start() {
    let _ = START.set(Instant::now());
}

/// Logs one startup phase with milliseconds since [`mark_start`].
pub fn phase(name: &str) {
    let ms = START
        .get()
        .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    tracing::info!(phase = name, ms, "startup");
}

/// Real apps and the fuzzy index built over their names; swapped together.
pub struct AppMatch {
    /// Entries, same order as the index.
    pub entries: Arc<Vec<AppEntry>>,
    /// Fuzzy index over entry names.
    pub index: MatchIndex,
}

/// Scanned programs and their own fuzzy index, kept apart so the Command budget never pays for them.
pub struct ProgramSet {
    /// Entries, same order as the index.
    pub entries: Vec<AppEntry>,
    /// Per entry: it sits on a removable drive (from the scan's drive kinds).
    pub removable: Vec<bool>,
    /// Fuzzy index over entry names.
    pub index: MatchIndex,
}

impl Default for ProgramSet {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            removable: Vec::new(),
            index: MatchIndex::new::<&str>(&[]),
        }
    }
}

/// Cancel flag of the current Find run; starting a run cancels the previous one.
#[derive(Default)]
pub struct FindRun(Mutex<Arc<AtomicBool>>);

impl FindRun {
    /// Cancels the previous run and returns the new run's flag.
    pub fn begin(&self) -> Arc<AtomicBool> {
        let mut cur = self.0.lock();
        cur.store(true, Ordering::Relaxed);
        *cur = Arc::new(AtomicBool::new(false));
        cur.clone()
    }

    /// Cancels the current run.
    pub fn cancel(&self) {
        self.0.lock().store(true, Ordering::Relaxed);
    }

    /// The current run's flag, without cancelling anything.
    pub fn current(&self) -> Arc<AtomicBool> {
        self.0.lock().clone()
    }
}

/// Single-flight flag of one background job: at most one run holds it.
#[derive(Default)]
pub struct Busy(Arc<AtomicBool>);

/// Held while a job runs; releases the job on drop, so a panic cannot wedge it.
pub struct BusyGuard(Arc<AtomicBool>);

impl Busy {
    /// Claims the job; `None` when a run already holds it.
    pub fn try_begin(&self) -> Option<BusyGuard> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| BusyGuard(Arc::clone(&self.0)))
    }

    /// Whether a run holds the job.
    pub fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Claims `busy` and returns the run's cancel flag. A superseding run (settings change, reindex,
/// feature on) cancels the current run and waits for it to let go; an on-open or startup run
/// skips when the job is held. Blocking: call from a blocking thread.
fn acquire(busy: &Busy, run: &FindRun, supersede: bool) -> Option<(BusyGuard, Arc<AtomicBool>)> {
    if !supersede {
        let guard = busy.try_begin()?;
        return Some((guard, run.current()));
    }
    let cancel = run.begin();
    loop {
        if let Some(guard) = busy.try_begin() {
            return Some((guard, cancel));
        }
        if cancel.load(Ordering::Relaxed) {
            return None; // superseded again, or the feature was turned off
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// When each index was last refreshed, in Unix seconds (0 = never), kept in memory. Seeded from
/// the cache headers and the snapshot mtime at startup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Freshness {
    /// Last apps refresh (or the apps cache mtime).
    pub apps: u64,
    /// Oldest scan among the cached program roots that are present.
    pub programs_full: u64,
    /// `(letter, serial)` of every root in the programs cache, attached or not.
    pub program_roots: Vec<(char, u32)>,
    /// `(letter, serial)` of the drives the published program set was filtered with.
    pub programs_present: Vec<(char, u32)>,
    /// Last Find scan (or the snapshot mtime).
    pub find_snapshot: u64,
    /// The Find watcher reported lost events (filled at plan time).
    pub find_stale: bool,
    /// Drive letters that are roots of the Find store (filled at plan time).
    pub find_drives: Vec<char>,
    /// The launcher has been opened in this process.
    pub opened: bool,
}

/// Settings and job state [`plan_refresh`] decides with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanCfg {
    /// Programs scan on.
    pub programs: bool,
    /// Find mode on.
    pub find: bool,
    /// Find indexes whole drives.
    pub find_drives: bool,
    /// System drive letter (Find covers it through the user folders).
    pub system: Option<char>,
    /// An apps refresh is running.
    pub busy_apps: bool,
    /// A programs run is running.
    pub busy_programs: bool,
    /// A Find run is running.
    pub busy_find: bool,
}

/// Work one launcher open triggers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshPlan {
    /// Refresh the apps list.
    pub apps: bool,
    /// Drive letters whose programs are rescanned (every present drive on a full rescan).
    pub program_roots: Vec<char>,
    /// The rescan is the 24 h full one.
    pub programs_full: bool,
    /// The set of present drives changed: republish (attached shown, detached hidden).
    pub programs_republish: bool,
    /// Rescan Find completely and restart the watcher.
    pub find_reconcile: bool,
    /// Drive roots to add to Find.
    pub find_roots_add: Vec<char>,
    /// Drive roots to drop from Find.
    pub find_roots_drop: Vec<char>,
}

impl RefreshPlan {
    fn programs(&self) -> bool {
        self.programs_full || !self.program_roots.is_empty() || self.programs_republish
    }
}

/// Decides what one launcher open refreshes. Pure. `drives` are the indexable drives as
/// `(letter, serial, kind)`; `now` is Unix seconds.
pub fn plan_refresh(
    now: u64,
    last: &Freshness,
    drives: &[(char, u32, DriveKind)],
    cfg: &PlanCfg,
) -> RefreshPlan {
    let old = |t: u64, max: u64| t == 0 || now.saturating_sub(t) > max;
    let mut plan = RefreshPlan {
        apps: !cfg.busy_apps && old(last.apps, APPS_MAX_AGE),
        ..RefreshPlan::default()
    };
    if cfg.programs && !cfg.busy_programs {
        let present: Vec<(char, u32)> = drives.iter().map(|&(l, s, _)| (l, s)).collect();
        plan.programs_full = old(last.programs_full, PROGRAMS_MAX_AGE);
        plan.program_roots = present
            .iter()
            .filter(|p| plan.programs_full || !last.program_roots.contains(p))
            .map(|&(l, _)| l)
            .collect();
        plan.programs_republish = present != last.programs_present;
    }
    if cfg.find && !cfg.busy_find {
        plan.find_reconcile =
            last.find_stale || (!last.opened && old(last.find_snapshot, FIND_MAX_AGE));
        if !plan.find_reconcile {
            // Find keeps fixed drives only until the entry cap lands (F8.14).
            let want: Vec<char> = if cfg.find_drives {
                drives
                    .iter()
                    .filter(|d| d.2 == DriveKind::Fixed && Some(d.0) != cfg.system)
                    .map(|d| d.0)
                    .collect()
            } else {
                Vec::new()
            };
            plan.find_roots_add = want
                .iter()
                .filter(|l| !last.find_drives.contains(l))
                .copied()
                .collect();
            plan.find_roots_drop = last
                .find_drives
                .iter()
                .filter(|l| !want.contains(l))
                .copied()
                .collect();
        }
    }
    plan
}

/// Letters of the whole-drive roots (path `X:\`, label `X:`) among Find's `roots`.
fn drive_letters(roots: &[Root]) -> Vec<char> {
    roots
        .iter()
        .filter_map(|r| {
            let s = r.path.to_str()?;
            let mut c = s.chars();
            let l = c.next()?;
            let whole = s.len() == 3 && c.next() == Some(':') && s.ends_with('\\');
            (whole && r.label.len() == 2).then_some(l.to_ascii_uppercase())
        })
        .collect()
}

/// Modification time of `path` in Unix seconds, 0 when unreadable.
fn mtime_secs(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Everything commands and tasks share.
pub struct AppState {
    /// Live config.
    pub config: Arc<ArcSwap<Config>>,
    /// SQLite store.
    pub store: Arc<Store>,
    /// Installed apps.
    pub apps: Arc<AppIndex>,
    /// Scanned programs plus their fuzzy index; empty while `apps.scan_programs` is off.
    pub programs: ArcSwap<ProgramSet>,
    /// Cancel flag of the programs scan (off, a new scan, or exit).
    pub programs_run: FindRun,
    programs_gate: Mutex<()>,
    /// Single-flight flags of the apps, programs and Find jobs and of the on-open refresh.
    apps_busy: Busy,
    programs_busy: Busy,
    find_busy: Busy,
    refresh_busy: Busy,
    /// When each index was last refreshed.
    fresh: Mutex<Freshness>,
    /// Apps plus fuzzy index.
    pub app_match: ArcSwap<AppMatch>,
    /// Reused matcher scratch for apps and features (keystroke thread).
    pub matcher: Mutex<FuzzyMatcher>,
    /// Matcher scratch for the programs match (blocking pool).
    pub prog_matcher: Mutex<FuzzyMatcher>,
    /// File index.
    pub files: Arc<FileIndex>,
    /// Live file changes.
    pub overlay: ArcSwap<Overlay>,
    /// File watcher, held while Find is on.
    pub watch: Mutex<Option<WatchHandle>>,
    /// Write-only secret vault (Credential Manager plus metadata).
    pub vault: Arc<SecretVault>,
    /// Cancels the running Find scan (on Find off, a new run, or exit).
    pub find_run: FindRun,
    /// Optional Spotify connection (also feeds the media feature).
    pub spotify: Arc<Spotify>,
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

/// Builds the fuzzy index for the real apps `entries`.
pub fn build_match(entries: Arc<Vec<AppEntry>>) -> AppMatch {
    let names: Vec<&str> = entries.iter().map(|a| a.name.as_str()).collect();
    let index = MatchIndex::new(&names);
    AppMatch { entries, index }
}

fn is_shortcut(p: &ProgramFile) -> bool {
    matches!(p.ext.as_str(), "lnk" | "url")
}

/// Programs as [`AppEntry`]s, minus any whose path an app already launches and any shortcut
/// named like a real app (Start Menu and Desktop shortcuts to apps already listed).
pub fn merge_programs(apps: &[AppEntry], programs: &[ProgramFile]) -> Vec<AppEntry> {
    let mut seen: std::collections::HashSet<String> = apps
        .iter()
        .flat_map(|a| {
            let t = a
                .target
                .as_ref()
                .map(|t| t.to_string_lossy().to_lowercase());
            [Some(a.parsing_name.to_lowercase()), t]
        })
        .flatten()
        .collect();
    let app_names: std::collections::HashSet<String> =
        apps.iter().map(|a| a.name.to_lowercase()).collect();
    let mut out = Vec::with_capacity(programs.len());
    for p in programs {
        if is_shortcut(p) && app_names.contains(&p.name.to_lowercase()) {
            continue;
        }
        let path = p.path.to_string_lossy().into_owned();
        if !seen.insert(path.to_lowercase()) {
            continue;
        }
        out.push(AppEntry {
            id: index_apps::make_id(&path),
            name: p.name.clone(),
            parsing_name: path, // the shell path; `target` stays None to save memory
            target: None,
            is_packaged: false,
        });
    }
    out
}

/// [`merge_programs`] plus the fuzzy index. `removable_letters` are the drive letters whose
/// `DriveKind` is `Removable` (from the drive scan); an entry is removable when its path starts there.
pub fn build_programs(
    apps: &[AppEntry],
    programs: &[ProgramFile],
    removable_letters: &[char],
) -> ProgramSet {
    let entries = merge_programs(apps, programs);
    let removable = entries
        .iter()
        .map(|e| {
            let mut c = e.parsing_name.chars();
            matches!((c.next(), c.next()), (Some(l), Some(':'))
                if removable_letters.contains(&l.to_ascii_uppercase()))
        })
        .collect();
    let names: Vec<&str> = entries.iter().map(|a| a.name.as_str()).collect();
    let index = MatchIndex::new(&names);
    ProgramSet {
        entries,
        removable,
        index,
    }
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

    fn programs_path(&self) -> PathBuf {
        self.cache_dir.join("programs.idx")
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
    let user: Vec<Root> = default_roots()
        .into_iter()
        .filter(|r| find.roots.iter().any(|n| n.eq_ignore_ascii_case(&r.label)))
        .collect();
    // Runs on the blocking pool: GetDriveTypeW can stall on network or optical drives.
    // Find keeps fixed drives only until the entry cap lands (F8.14).
    let drive_list: Vec<char> = if find.include_drives {
        win::drives::indexable_drives()
            .into_iter()
            .filter(|d| d.kind == win::drives::DriveKind::Fixed)
            .map(|d| d.letter)
            .collect()
    } else {
        Vec::new()
    };
    let extra = find
        .extra_roots
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    drives::resolve_roots(
        user,
        &drive_list,
        drives::system_drive(),
        find.include_drives,
        extra,
    )
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
    phase("config loaded");
    let data = local_data_dir().context("locating data dir")?;
    std::fs::create_dir_all(&data).context("creating data dir")?;
    let (store, report) = Store::open(&data.join("store.db")).context("opening store")?;
    let store = Arc::new(store);
    phase("store opened");
    if report.reset
        && let Err(err) = handle.emit("store://reset", serde_json::json!({}))
    {
        tracing::warn!(%err, "emit failed");
    }
    let cache_dir = cache::cache_dir().context("locating cache dir")?;
    std::fs::create_dir_all(&cache_dir).context("creating cache dir")?;
    let apps = Arc::new(AppIndex::new());
    let apps_loaded = apps.load_cache(&cache_dir).unwrap_or_else(|err| {
        tracing::warn!(%err, "app cache unreadable");
        false
    });
    let apps_stamp = if apps_loaded {
        mtime_secs(&cache_dir.join("apps.json")).max(1)
    } else {
        0
    };
    phase("app cache loaded");
    let vault = Arc::new(SecretVault::new(
        Arc::new(KeyringBackend),
        Arc::new(JsonMeta::open(data.join("secret_meta.json"))),
    ));
    let keys: Arc<dyn KeySource> = Arc::new(VaultKeys(vault.clone()));
    let spotify =
        Arc::new(Spotify::new(vault.clone(), config.clone()).context("building spotify client")?);
    let lost = handle.clone();
    spotify.on_lost(move || {
        let payload = serde_json::json!({ "connected": false, "name": serde_json::Value::Null });
        if let Err(err) = lost.emit("spotify://state", payload) {
            tracing::warn!(%err, "emit failed");
        }
    });
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
        programs: ArcSwap::from_pointee(ProgramSet::default()),
        programs_run: FindRun::default(),
        programs_gate: Mutex::new(()),
        apps_busy: Busy::default(),
        programs_busy: Busy::default(),
        find_busy: Busy::default(),
        refresh_busy: Busy::default(),
        fresh: Mutex::new(Freshness {
            apps: apps_stamp,
            ..Freshness::default()
        }),
        prog_matcher: Mutex::new(FuzzyMatcher::new()),
        apps,
        matcher: Mutex::new(FuzzyMatcher::new()),
        files: Arc::new(FileIndex::new()),
        overlay: ArcSwap::from_pointee(Overlay::new()),
        watch: Mutex::new(None),
        vault,
        spotify,
        find_run: FindRun::default(),
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
    phase("state init done");
    Ok(())
}

/// Spawns background startup work; returns immediately. Caches are published without walks; an
/// index is built at startup only when it has no cache (first run), serially: apps, Find, programs.
pub fn start(app: &AppHandle) {
    let handle = app.clone();
    let state: Arc<AppState> = Arc::clone(&*app.state::<Arc<AppState>>());
    let s = state.clone();
    tauri::async_runtime::spawn(async move {
        for f in builtin_features_with(
            Media::with_api(s.spotify.clone()),
            features::capture::Capture::new(s.vault.clone()),
        ) {
            s.registry.register(f).await;
        }
        let host = Arc::new(crate::nlhost::ShellHost(Arc::downgrade(&s)));
        s.registry
            .register(Arc::new(features::NlFind::new(host)))
            .await;
        let cfg = s.config.load_full();
        s.registry.apply_config(&cfg, &s.ctx).await;
        if let Err(err) = crate::hotkey::sync_explain(&handle, &cfg.general.explain_hotkey) {
            tracing::warn!(%err.message, "explain hotkey not bound");
        }
    });
    let s = state;
    tauri::async_runtime::spawn(async move {
        let first_run = s.fresh.lock().apps == 0;
        if first_run {
            refresh_apps(&s).await;
        } else {
            warm_icons(&s).await;
        }
        if s.config.load().modes.find.enabled {
            load_find(&s).await;
        }
        run_programs(&s, false, Vec::new(), false).await;
    });
}

/// Reads the cached app icons into memory (no extraction, no walk).
async fn warm_icons(s: &Arc<AppState>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        st.icons.warm(&st.apps.entries(), crate::icons::ICON_PX);
    })
    .await;
    if let Err(err) = res {
        tracing::warn!(%err, "icon warm task failed");
    }
}

/// Called after the launcher is shown: spawns the on-open refresh and returns at once, so the
/// show path never waits on it. One refresh at a time; an open during a refresh adds nothing.
pub fn on_open(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let s = Arc::clone(&*state);
    tauri::async_runtime::spawn(async move { refresh_on_open(&s).await });
}

async fn refresh_on_open(s: &Arc<AppState>) {
    let Some(_run) = s.refresh_busy.try_begin() else {
        return;
    };
    let st = s.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || {
        // A few syscalls per letter; never touches remote or optical drives (D6).
        let vols: Vec<(char, u32, DriveKind)> = win::drives::indexable_drives()
            .into_iter()
            .map(|d| (d.letter, d.serial, d.kind))
            .collect();
        let c = st.config.load();
        let cfg = PlanCfg {
            programs: c.apps.scan_programs,
            find: c.modes.find.enabled,
            find_drives: c.modes.find.include_drives,
            system: drives::system_drive(),
            busy_apps: st.apps_busy.is_busy(),
            busy_programs: st.programs_busy.is_busy(),
            busy_find: st.find_busy.is_busy(),
        };
        let stale = st.watch.lock().as_ref().is_some_and(WatchHandle::is_stale);
        let find_drives = drive_letters(st.files.load().roots());
        let mut f = st.fresh.lock();
        f.find_stale = stale;
        f.find_drives = find_drives;
        let plan = plan_refresh(unix_now(), &f, &vols, &cfg);
        f.opened = true;
        plan
    })
    .await;
    let plan = match plan {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(%err, "refresh plan task failed");
            return;
        }
    };
    // Serial, like startup: one walk at a time.
    if plan.apps {
        refresh_apps(s).await;
    }
    if plan.find_reconcile {
        find_full(s, false).await;
    } else if !plan.find_roots_add.is_empty() || !plan.find_roots_drop.is_empty() {
        find_drive_roots(s, plan.find_roots_add.clone(), plan.find_roots_drop.clone()).await;
    }
    if plan.programs() {
        run_programs(s, plan.programs_full, plan.program_roots, false).await;
    }
}

/// Refreshes the app list and extracts the icons missing on disk. Skips when one is running.
pub(crate) async fn refresh_apps(s: &Arc<AppState>) {
    let Some(busy) = s.apps_busy.try_begin() else {
        tracing::debug!("apps refresh already running");
        return;
    };
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        let stats = st.apps.refresh_blocking()?;
        st.apps.save_cache(&st.cache_dir)?;
        st.rebuild_match();
        // Icons after the list is usable; ensure skips files that exist.
        let _com = index_apps::com::ComGuard::new().map_err(index_apps::AppsError::Enumerate)?;
        let entries = st.apps.entries();
        for e in entries.iter() {
            if let Err(err) = st.icons.cache().ensure(e) {
                tracing::debug!(%err, app = %e.name, "icon extraction failed");
            }
        }
        st.icons.warm(&entries, crate::icons::ICON_PX);
        Ok::<_, index_apps::AppsError>(stats)
    })
    .await;
    // Stamped on failure too, so a broken source is retried in 10 min, not on every open.
    s.fresh.lock().apps = unix_now();
    match res {
        Ok(Ok(stats)) => tracing::info!(kept = stats.kept, "apps refreshed"),
        Ok(Err(err)) => tracing::warn!(%err, "app refresh failed"),
        Err(err) => tracing::warn!(%err, "app refresh task failed"),
    }
}

/// Loads the programs cache, scans what is asked, saves and publishes the entries of the drives
/// that are attached. Scans every drive when `all` or when there is no cache, else only the drive
/// letters in `roots`. Off when `apps.scan_programs` is false; [`stop_programs`] cancels it.
/// `supersede` (settings, reindex) cancels a running run; otherwise a running run makes this skip.
pub(crate) async fn run_programs(s: &Arc<AppState>, all: bool, roots: Vec<char>, supersede: bool) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.programs_busy, &st.programs_run, supersede) else {
            return Err(FilesError::Cancelled);
        };
        let live = || !cancel.load(Ordering::Relaxed) && st.config.load().apps.scan_programs;
        if !live() {
            return Err(FilesError::Cancelled);
        }
        let vols = win::drives::indexable_drives();
        let present: Vec<(char, u32)> = vols.iter().map(|d| (d.letter, d.serial)).collect();
        let removable: Vec<char> = vols
            .iter()
            .filter(|d| d.kind == DriveKind::Removable)
            .map(|d| d.letter)
            .collect();
        // Cancel is set before `stop_programs` takes the gate, so a store here either lands first
        // (and is cleared) or sees the flag.
        let publish = |cache: &ProgramCache| {
            let set = build_programs(&st.apps.entries(), &cache.visible(&present), &removable);
            let _gate = st.programs_gate.lock();
            if live() {
                st.programs.store(Arc::new(set));
            }
        };
        let path = st.programs_path();
        let (mut cache, cached) = match ProgramCache::load(&path) {
            Ok(Some(c)) => (c, true),
            Ok(None) => (ProgramCache::default(), false),
            Err(err) => {
                tracing::warn!(%err, "programs cache unreadable");
                (ProgramCache::default(), false)
            }
        };
        let targets: Vec<(PathBuf, u32)> = vols
            .iter()
            .filter(|d| all || !cached || roots.contains(&d.letter))
            .map(|d| (PathBuf::from(format!("{}:\\", d.letter)), d.serial))
            .collect();
        let mut scanned = None;
        if !targets.is_empty() {
            if supersede && cached {
                publish(&cache); // usable at once while the rescan runs
            }
            let started = Instant::now();
            let failed = {
                let pool = scan_pool();
                let _bg = win::thread::BackgroundGuard::new();
                cache.scan_into(&targets, &pool, &cancel)?
            };
            for (root, err) in &failed {
                tracing::warn!(root = %root.display(), %err, "programs root not scanned");
            }
            if !live() {
                return Err(FilesError::Cancelled);
            }
            if let Err(err) = cache.save(&path) {
                tracing::warn!(%err, "saving programs cache failed");
            }
            scanned = Some((targets.len(), started.elapsed().as_millis() as u64));
        }
        publish(&cache);
        let oldest = cache
            .roots
            .iter()
            .filter(|r| r.letter().is_some_and(|l| present.contains(&(l, r.serial))))
            .map(|r| r.scanned)
            .min()
            .unwrap_or(0);
        let program_roots = cache
            .roots
            .iter()
            .filter_map(|r| Some((r.letter()?, r.serial)))
            .collect();
        let count = cache.files.len();
        let mut f = st.fresh.lock();
        f.programs_full = oldest;
        f.program_roots = program_roots;
        f.programs_present = present;
        Ok((scanned, count))
    })
    .await;
    match res {
        Ok(Ok((Some((roots, ms)), count))) => tracing::info!(roots, count, ms, "programs scanned"),
        Ok(Ok((None, _))) => {}
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("programs run cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "programs scan failed"),
        Err(err) => tracing::warn!(%err, "programs scan task failed"),
    }
}

/// Programs off: cancels a running scan and frees the program entries and index.
pub(crate) fn stop_programs(s: &AppState) {
    s.programs_run.cancel();
    let _gate = s.programs_gate.lock();
    s.programs.store(Arc::new(ProgramSet::default()));
}

/// Two background-mode workers for one run. The pool is dropped with the returned value when the
/// run ends or is cancelled, so no scan thread outlives its run.
fn scan_pool() -> index_files::Parallelism {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .thread_name(|i| format!("find-scan-{i}"))
        .start_handler(|_| {
            if let Err(err) = win::thread::enter_background_mode() {
                tracing::debug!(%err, "scan worker background mode unavailable");
            }
        })
        .build();
    match pool {
        Ok(pool) => index_files::Parallelism::RayonExistingPool {
            pool: Arc::new(pool),
            busy_timeout: None,
        },
        Err(err) => {
            tracing::warn!(%err, "scan pool failed; scanning serially");
            index_files::Parallelism::Serial
        }
    }
}

/// Starts watching `roots` and publishes the watcher's overlay. Roots that cannot be watched
/// (drive gone) are logged and skipped.
fn watch_roots(
    st: &AppState,
    watch: &mut Option<WatchHandle>,
    roots: &[Root],
) -> Result<(), FilesError> {
    *watch = None; // the old watcher stops before the new one starts
    let handle = st.files.watch_auto(roots)?;
    for (root, why) in handle.failed() {
        tracing::warn!(root = %root.path.display(), why, "find root not watched");
    }
    st.overlay.store(handle.overlay());
    *watch = Some(handle);
    Ok(())
}

/// Startup with Find on: publishes the snapshot and watches its roots, with no walk. Without a
/// snapshot (first run) it scans once.
async fn load_find(s: &Arc<AppState>) {
    let st = s.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.find_busy, &st.find_run, false) else {
            return true; // another run builds the index
        };
        let store = match snapshot::load(&st.snapshot_path()) {
            Ok(Some(store)) => store,
            Ok(None) => return false,
            Err(err) => {
                tracing::warn!(%err, "file snapshot unreadable");
                return false;
            }
        };
        // stop_find cancels, then takes this lock: checking under it leaves no window.
        let mut watch = st.watch.lock();
        if cancel.load(Ordering::Relaxed) || !st.config.load().modes.find.enabled {
            return true;
        }
        let roots = store.roots().to_vec();
        st.files.replace(store);
        if let Err(err) = watch_roots(&st, &mut watch, &roots) {
            tracing::warn!(%err, "find watcher failed");
        }
        drop(watch);
        st.fresh.lock().find_snapshot = mtime_secs(&st.snapshot_path()).max(1);
        true
    })
    .await
    .unwrap_or_else(|err| {
        tracing::warn!(%err, "find load task failed");
        true
    });
    if !loaded {
        find_full(s, false).await;
    }
}

/// Full Find scan (Find on, a roots change or a reindex): supersedes a running Find run.
pub(crate) async fn start_find(s: &Arc<AppState>) {
    find_full(s, true).await;
}

/// Scans every Find root, publishes and saves the store and restarts the watcher. Until the scan
/// ends, an empty index shows the snapshot.
async fn find_full(s: &Arc<AppState>, supersede: bool) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.find_busy, &st.find_run, supersede) else {
            return Err(FilesError::Cancelled);
        };
        let live = || !cancel.load(Ordering::Relaxed) && st.config.load().modes.find.enabled;
        if !live() {
            return Err(FilesError::Cancelled);
        }
        if st.files.is_empty() {
            match snapshot::load(&st.snapshot_path()) {
                Ok(Some(store)) => {
                    // Check and replace under the watch lock stop_find takes after cancelling.
                    let _watch = st.watch.lock();
                    if live() {
                        st.files.replace(store);
                    }
                }
                Ok(None) => {}
                Err(err) => tracing::warn!(%err, "file snapshot unreadable"),
            }
        }
        let roots = roots_for(&st.config.load());
        let started = Instant::now();
        let store = {
            let pool = scan_pool();
            let _bg = win::thread::BackgroundGuard::new();
            scan(&roots, &pool, &cancel, |_| {})?
        };
        tracing::info!(
            files = store.len(),
            roots = roots.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "find scan finished"
        );
        // stop_find cancels, then takes this lock: checking under it leaves no window.
        let mut watch = st.watch.lock();
        if !live() {
            return Err(FilesError::Cancelled);
        }
        st.files.replace(store);
        st.save_snapshot();
        watch_roots(&st, &mut watch, &roots)?;
        drop(watch);
        st.fresh.lock().find_snapshot = unix_now();
        Ok(())
    })
    .await;
    match res {
        Ok(Ok(())) => tracing::info!("find index ready"),
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("find run cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "find index failed"),
        Err(err) => tracing::warn!(%err, "find task failed"),
    }
}

/// Adds the drive roots `add` (one walk each) and drops `drop_roots` (no I/O), keeping the rest
/// of the store, then saves the snapshot and restarts the watcher on the new roots.
async fn find_drive_roots(s: &Arc<AppState>, add: Vec<char>, drop_roots: Vec<char>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.find_busy, &st.find_run, false) else {
            return Err(FilesError::Cancelled);
        };
        let live = || !cancel.load(Ordering::Relaxed) && st.config.load().modes.find.enabled;
        let mut scanned = Vec::new();
        if !add.is_empty() {
            let pool = scan_pool();
            let _bg = win::thread::BackgroundGuard::new();
            for l in &add {
                let root = Root {
                    id: 0,
                    path: PathBuf::from(format!("{l}:\\")),
                    label: format!("{l}:"),
                };
                let store = scan(std::slice::from_ref(&root), &pool, &cancel, |_| {})?;
                scanned.push((root, store));
            }
        }
        let mut watch = st.watch.lock();
        if !live() {
            return Err(FilesError::Cancelled);
        }
        // Fold pending watcher changes in before the store is rebuilt from it.
        if let Some(h) = watch.take() {
            st.files.compact(&h.overlay());
        }
        let cur = st.files.load();
        let mut next: Option<index_files::FileStore> = None;
        for l in &drop_roots {
            let n = next
                .as_ref()
                .unwrap_or(&*cur)
                .without_root(std::path::Path::new(&format!("{l}:\\")));
            next = Some(n);
        }
        for (root, store) in &scanned {
            let added = next.as_ref().unwrap_or(&*cur).with_root(root, store);
            match added {
                Ok(n) => next = Some(n),
                Err(err) => tracing::warn!(%err, "find drive root not added"),
            }
        }
        if let Some(n) = next {
            st.files.replace(n);
            st.save_snapshot();
        }
        let roots = st.files.load().roots().to_vec();
        watch_roots(&st, &mut watch, &roots)?;
        Ok((add.len(), drop_roots.len()))
    })
    .await;
    match res {
        Ok(Ok((added, dropped))) => tracing::info!(added, dropped, "find drive roots updated"),
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("find drive update cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "find drive update failed"),
        Err(err) => tracing::warn!(%err, "find drive task failed"),
    }
}

/// Find off: drops the watcher and frees the file index memory.
pub(crate) fn stop_find(s: &AppState) {
    s.find_run.cancel();
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
    state.find_run.cancel();
    state.programs_run.cancel();
    state.watch.lock().take();
    tauri::async_runtime::block_on(state.registry.stop_all());
    state.store.shutdown();
    if let Err(err) = state.vault.flush() {
        tracing::warn!(%err, "secret metadata flush failed");
    }
    state.save_snapshot();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn entry(name: &str, parsing: &str, target: Option<&str>) -> AppEntry {
        AppEntry {
            id: name.into(),
            name: name.into(),
            parsing_name: parsing.into(),
            target: target.map(PathBuf::from),
            is_packaged: false,
        }
    }

    fn prog(path: &str) -> ProgramFile {
        ProgramFile::from_path(Path::new(path)).unwrap()
    }

    #[test]
    fn merge_drops_duplicates_and_app_shortcuts() {
        let apps = vec![
            entry("A", "shell.A", Some(r"C:\x\a.exe")),
            entry("B", r"C:\x\b.lnk", None),
            entry("Notepad", "Microsoft.Notepad!App", None),
        ];
        let progs = vec![
            prog(r"C:\X\A.EXE"),
            prog(r"C:\x\b.lnk"),
            prog(r"C:\x\c.msi"),
            prog(r"C:\x\c.msi"),
            prog(r"C:\Users\me\Desktop\notepad.LNK"), // shortcut named like an app
            prog(r"C:\tools\notepad.exe"),            // same name but a real exe: kept
        ];
        let out = merge_programs(&apps, &progs);
        let names: Vec<_> = out.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["c", "notepad"]);
        assert_eq!(out[0].parsing_name, r"C:\x\c.msi");
        assert!(out[0].target.is_none());
        assert_eq!(out[0].id, index_apps::make_id(r"C:\x\c.msi"));
    }

    #[test]
    fn removable_flag_follows_drive_kind() {
        let progs = vec![prog(r"E:\a.exe"), prog(r"c:\b.exe"), prog(r"e:\c.exe")];
        let set = build_programs(&[], &progs, &['E']);
        assert_eq!(set.removable, [true, false, true]);
        assert_eq!(set.removable.len(), set.entries.len());
    }

    #[test]
    fn empty_program_set_is_empty() {
        let set = ProgramSet::default();
        assert!(set.entries.is_empty() && set.index.is_empty());
    }

    #[test]
    fn begin_cancels_previous_run() {
        let run = FindRun::default();
        let a = run.begin();
        assert!(!a.load(Ordering::Relaxed));
        let b = run.begin();
        assert!(a.load(Ordering::Relaxed));
        assert!(!b.load(Ordering::Relaxed));
        run.cancel();
        assert!(b.load(Ordering::Relaxed));
        assert!(run.current().load(Ordering::Relaxed));
    }

    const NOW: u64 = 1_000_000;
    const MIN: u64 = 60;
    const C: (char, u32, DriveKind) = ('C', 0xC0, DriveKind::Fixed);
    const D: (char, u32, DriveKind) = ('D', 0xD0, DriveKind::Fixed);
    const E: (char, u32, DriveKind) = ('E', 0xE0, DriveKind::Removable);

    fn cfg() -> PlanCfg {
        PlanCfg {
            programs: true,
            find: true,
            find_drives: true,
            system: Some('C'),
            ..PlanCfg::default()
        }
    }

    /// Everything refreshed a minute ago with C: and D: attached, after the first open.
    fn fresh() -> Freshness {
        Freshness {
            apps: NOW - MIN,
            programs_full: NOW - MIN,
            program_roots: vec![('C', 0xC0), ('D', 0xD0)],
            programs_present: vec![('C', 0xC0), ('D', 0xD0)],
            find_snapshot: NOW - MIN,
            find_stale: false,
            find_drives: vec!['D'],
            opened: true,
        }
    }

    #[test]
    fn plan_refresh_table() {
        let both = [C, D];
        let cases = vec![
            (
                "fresh caches",
                fresh(),
                both.to_vec(),
                cfg(),
                RefreshPlan::default(),
            ),
            (
                "apps 11 min old",
                Freshness {
                    apps: NOW - 11 * MIN,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    apps: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "new removable volume",
                fresh(),
                vec![C, D, E],
                cfg(),
                RefreshPlan {
                    program_roots: vec!['E'],
                    programs_republish: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "same letter, new serial",
                fresh(),
                vec![C, ('D', 0xD1, DriveKind::Fixed)],
                cfg(),
                RefreshPlan {
                    program_roots: vec!['D'],
                    programs_republish: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "detached volume",
                fresh(),
                vec![C],
                cfg(),
                RefreshPlan {
                    programs_republish: true,
                    find_roots_drop: vec!['D'],
                    ..RefreshPlan::default()
                },
            ),
            (
                "programs 25 h old",
                Freshness {
                    programs_full: NOW - 25 * 3600,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    program_roots: vec!['C', 'D'],
                    programs_full: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "first open, 1 h old snapshot",
                Freshness {
                    find_snapshot: NOW - 60 * MIN,
                    opened: false,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    find_reconcile: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "later open, 1 h old snapshot",
                Freshness {
                    find_snapshot: NOW - 60 * MIN,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan::default(),
            ),
            (
                "stale watcher",
                Freshness {
                    find_stale: true,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    find_reconcile: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "jobs already running",
                Freshness {
                    apps: 0,
                    programs_full: 0,
                    find_stale: true,
                    ..fresh()
                },
                vec![C, E],
                PlanCfg {
                    busy_apps: true,
                    busy_programs: true,
                    busy_find: true,
                    ..cfg()
                },
                RefreshPlan::default(),
            ),
            (
                "features off",
                Freshness {
                    find_stale: true,
                    ..fresh()
                },
                vec![C, E],
                PlanCfg {
                    programs: false,
                    find: false,
                    ..cfg()
                },
                RefreshPlan::default(),
            ),
            (
                "include_drives turned off drops drive roots",
                fresh(),
                both.to_vec(),
                PlanCfg {
                    find_drives: false,
                    ..cfg()
                },
                RefreshPlan {
                    find_roots_drop: vec!['D'],
                    ..RefreshPlan::default()
                },
            ),
        ];
        for (name, last, drives, cfg, want) in cases {
            assert_eq!(plan_refresh(NOW, &last, &drives, &cfg), want, "{name}");
        }
    }

    #[test]
    fn new_fixed_drive_joins_find() {
        let plan = plan_refresh(
            NOW,
            &fresh(),
            &[C, D, ('F', 0xF0, DriveKind::Fixed)],
            &cfg(),
        );
        assert_eq!(plan.find_roots_add, ['F']);
        assert_eq!(plan.program_roots, ['F']);
        assert!(plan.find_roots_drop.is_empty());
    }

    #[test]
    fn busy_is_single_flight_and_panic_safe() {
        let busy = Busy::default();
        let guard = busy.try_begin();
        assert!(guard.is_some() && busy.is_busy());
        assert!(busy.try_begin().is_none());
        drop(guard);
        assert!(!busy.is_busy());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = busy.try_begin();
            panic!("job failed");
        }));
        assert!(caught.is_err());
        assert!(busy.try_begin().is_some());
    }

    #[test]
    fn acquire_skips_when_held_and_supersede_waits() {
        let busy = Busy::default();
        let run = FindRun::default();
        let held = acquire(&busy, &run, false);
        assert!(held.is_some());
        assert!(acquire(&busy, &run, false).is_none());
        // A superseding run cancels the holder's flag, then claims the job once it lets go.
        let (guard, first) = held.unwrap();
        let busy = Arc::new(busy);
        let run = Arc::new(run);
        let (b, r) = (busy.clone(), run.clone());
        let next = std::thread::spawn(move || acquire(&b, &r, true).map(|(_, c)| c));
        while !first.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        drop(guard);
        let cancel = next.join().unwrap().unwrap();
        assert!(!cancel.load(Ordering::Relaxed));
        assert!(!busy.is_busy(), "the superseding run's guard was dropped");
    }

    #[test]
    fn drive_letters_picks_whole_drive_roots() {
        let root = |p: &str, l: &str| Root {
            id: 0,
            path: p.into(),
            label: l.into(),
        };
        let roots = [
            root(r"C:\Users\me\Desktop", "Desktop"),
            root(r"D:\", "D:"),
            root(r"E:\", r"E:\"), // an extra root that happens to be a drive
        ];
        assert_eq!(drive_letters(&roots), ['D']);
    }
}
