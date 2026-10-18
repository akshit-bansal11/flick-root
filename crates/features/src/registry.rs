//! Feature registry and lifecycle state machine (BSD section 3).

use crate::ctx::{FeatureCtx, TaskScope};
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{SettingsSchema, validate};
use arc_swap::ArcSwap;
use launcher_core::{Config, Input, Item};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Lifecycle state of one feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureState {
    /// Known, not yet started or stopped by config.
    Registered,
    /// `start()` running.
    Starting,
    /// Started; queried on every keystroke.
    Running,
    /// `stop()` running.
    Stopping,
    /// Off; holds no tasks.
    Stopped,
    /// `start()` failed; skipped by `query_all`.
    Failed,
}

struct Entry {
    feature: Arc<dyn Feature>,
    state: FeatureState,
    scope: Option<TaskScope>,
    error: Option<String>,
}

/// What the Features page shows for one feature.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureView {
    /// Feature id.
    pub id: &'static str,
    /// Title.
    pub title: &'static str,
    /// Version tag.
    pub version_tag: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// True when on (running, starting or failed to start).
    pub enabled: bool,
    /// Lifecycle state.
    pub status: FeatureState,
    /// Start error, if failed.
    pub error: Option<String>,
    /// Options schema.
    pub schema: SettingsSchema,
    /// Current `[features.<id>]` table.
    pub options: toml::Table,
}

/// Owns all features. Toggles are serialized by one async lock; the keystroke path reads a lock-free snapshot.
#[derive(Default)]
pub struct Registry {
    entries: RwLock<Vec<Entry>>,
    running: ArcSwap<Vec<Arc<dyn Feature>>>,
}

fn wanted(cfg: &Config, f: &dyn Feature) -> bool {
    cfg.features
        .get(f.id())
        .and_then(|t| t.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or_else(|| f.default_enabled())
}

/// Runs `start()` in a fresh child scope. A failed start has its scope shut down.
async fn start_entry(e: &mut Entry, ctx: &FeatureCtx) {
    e.state = FeatureState::Starting;
    let scope = ctx.tasks.child();
    let mut c = ctx.clone();
    c.tasks = scope.clone();
    match e.feature.start(c) {
        Ok(()) => {
            e.state = FeatureState::Running;
            e.scope = Some(scope);
            e.error = None;
        }
        Err(err) => {
            tracing::warn!(id = e.feature.id(), %err, "feature failed to start");
            e.state = FeatureState::Failed;
            e.error = Some(err.to_string());
            scope.shutdown().await;
        }
    }
}

async fn stop_entry(e: &mut Entry) {
    e.state = FeatureState::Stopping;
    e.feature.stop();
    if let Some(s) = e.scope.take() {
        s.shutdown().await;
    }
    e.error = None;
    e.state = FeatureState::Stopped;
}

fn find<'a>(v: &'a mut [Entry], id: &str) -> Result<&'a mut Entry, FeatureError> {
    v.iter_mut()
        .find(|e| e.feature.id() == id)
        .ok_or_else(|| FeatureError::Unknown(id.to_string()))
}

impl Registry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    fn publish(&self, v: &[Entry]) {
        let run = v
            .iter()
            .filter(|e| e.state == FeatureState::Running)
            .map(|e| e.feature.clone())
            .collect();
        self.running.store(Arc::new(run));
    }

    /// Adds a feature in the `Registered` state.
    pub async fn register(&self, feature: Arc<dyn Feature>) {
        self.entries.write().await.push(Entry {
            feature,
            state: FeatureState::Registered,
            scope: None,
            error: None,
        });
    }

    /// Starts features the config enables and stops those it disables. Failed features are left alone unless disabled.
    pub async fn apply_config(&self, cfg: &Config, ctx: &FeatureCtx) {
        let mut v = self.entries.write().await;
        for e in v.iter_mut() {
            let want = wanted(cfg, e.feature.as_ref());
            match (want, e.state) {
                (true, FeatureState::Registered | FeatureState::Stopped) => {
                    start_entry(e, ctx).await;
                }
                (false, FeatureState::Running | FeatureState::Failed) => stop_entry(e).await,
                (false, FeatureState::Registered) => e.state = FeatureState::Stopped,
                _ => {}
            }
        }
        self.publish(&v);
    }

    /// Turns one feature on or off. Returns the updated config for the caller to persist.
    pub async fn set_enabled(
        &self,
        id: &str,
        on: bool,
        ctx: &FeatureCtx,
    ) -> Result<Config, FeatureError> {
        let mut v = self.entries.write().await;
        let e = find(&mut v, id)?;
        let mut cfg = (**ctx.config.load()).clone();
        cfg.features
            .entry(id.to_string())
            .or_default()
            .insert("enabled".into(), toml::Value::Boolean(on));
        ctx.config.store(Arc::new(cfg.clone()));
        match (on, e.state) {
            (true, FeatureState::Registered | FeatureState::Stopped | FeatureState::Failed) => {
                start_entry(e, ctx).await;
            }
            (false, FeatureState::Running | FeatureState::Failed) => stop_entry(e).await,
            (false, FeatureState::Registered) => e.state = FeatureState::Stopped,
            _ => {}
        }
        self.publish(&v);
        Ok(cfg)
    }

    /// Validates and applies new options (`enabled` is kept). Restarts only when the feature says so. Returns the updated config for the caller to persist.
    pub async fn set_options(
        &self,
        id: &str,
        mut table: toml::Table,
        ctx: &FeatureCtx,
    ) -> Result<Config, FeatureError> {
        let mut v = self.entries.write().await;
        let e = find(&mut v, id)?;
        validate(&e.feature.settings_schema(), &table)?;
        let mut cfg = (**ctx.config.load()).clone();
        let old = cfg.features.get(id).cloned().unwrap_or_default();
        if let Some(en) = old.get("enabled") {
            table.insert("enabled".into(), en.clone());
        }
        let changed: Vec<String> = old
            .keys()
            .chain(table.keys())
            .filter(|k| old.get(*k) != table.get(*k))
            .cloned()
            .collect();
        cfg.features.insert(id.to_string(), table);
        ctx.config.store(Arc::new(cfg.clone()));
        if e.state == FeatureState::Running && e.feature.needs_restart_for(&changed) {
            stop_entry(e).await;
            start_entry(e, ctx).await;
        }
        self.publish(&v);
        Ok(cfg)
    }

    /// Items from running features whose modes include `input.mode`. Sync, takes no async lock.
    pub fn query_all(&self, input: &Input) -> Vec<Item> {
        let mut out = Vec::new();
        for f in self.running.load().iter() {
            if f.modes().contains(&input.mode) {
                out.extend(f.query(input));
            }
        }
        out
    }

    /// Sum of running features' rank boosts for `item`.
    pub fn rank_boost_all(&self, input: &Input, item: &Item) -> f32 {
        self.running
            .load()
            .iter()
            .filter(|f| f.modes().contains(&input.mode))
            .map(|f| f.rank_boost(input, item))
            .sum()
    }

    /// Tells running features the user chose `item`.
    pub fn on_chosen_all(&self, input: &Input, item: &Item) {
        for f in self.running.load().iter() {
            f.on_chosen(input, item);
        }
    }

    /// Looks up a feature by id, for `execute`.
    pub async fn get(&self, id: &str) -> Option<Arc<dyn Feature>> {
        let v = self.entries.read().await;
        v.iter()
            .find(|e| e.feature.id() == id)
            .map(|e| e.feature.clone())
    }

    /// One view per feature, in registration order.
    pub async fn views(&self, cfg: &Config) -> Vec<FeatureView> {
        let v = self.entries.read().await;
        v.iter()
            .map(|e| FeatureView {
                id: e.feature.id(),
                title: e.feature.title(),
                version_tag: e.feature.version_tag(),
                description: e.feature.description(),
                enabled: matches!(
                    e.state,
                    FeatureState::Running | FeatureState::Starting | FeatureState::Failed
                ),
                status: e.state,
                error: e.error.clone(),
                schema: e.feature.settings_schema(),
                options: cfg
                    .features
                    .get(e.feature.id())
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// `(running, total)`.
    pub async fn on_count(&self) -> (usize, usize) {
        let v = self.entries.read().await;
        let on = v
            .iter()
            .filter(|e| e.state == FeatureState::Running)
            .count();
        (on, v.len())
    }

    /// Stops every feature (app quit).
    pub async fn stop_all(&self) {
        let mut v = self.entries.write().await;
        for e in v.iter_mut() {
            if matches!(e.state, FeatureState::Running | FeatureState::Failed) {
                stop_entry(e).await;
            }
        }
        self.publish(&v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::EventSink;
    use crate::schema::{Field, FieldKind};
    use async_trait::async_trait;
    use launcher_core::store::Store;
    use launcher_core::{Action, ItemId, ItemKind, Mode, Outcome};
    use providers::hub::ProviderHub;

    struct Sink;
    impl EventSink for Sink {
        fn emit(&self, _: &str, _: serde_json::Value) {}
    }

    struct Stub {
        id: &'static str,
        fail: bool,
        mode: &'static [Mode],
    }

    #[async_trait]
    impl Feature for Stub {
        fn id(&self) -> &'static str {
            self.id
        }
        fn title(&self) -> &'static str {
            "Stub"
        }
        fn version_tag(&self) -> &'static str {
            "v0.0.0"
        }
        fn description(&self) -> &'static str {
            "stub"
        }
        fn default_enabled(&self) -> bool {
            true
        }
        fn modes(&self) -> &'static [Mode] {
            self.mode
        }
        fn settings_schema(&self) -> SettingsSchema {
            SettingsSchema {
                fields: vec![Field {
                    key: "n".into(),
                    label: "n".into(),
                    help: String::new(),
                    kind: FieldKind::Bool,
                    default: serde_json::json!(false),
                    restart: true,
                }],
            }
        }
        fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
            if self.fail {
                return Err(FeatureError::Start {
                    id: self.id,
                    reason: "boom".into(),
                });
            }
            let t = ctx.tasks.token();
            ctx.tasks.spawn(async move { t.cancelled().await });
            Ok(())
        }
        fn stop(&self) {}
        fn query(&self, _: &Input) -> Vec<Item> {
            vec![Item {
                id: ItemId(self.id.into()),
                kind: ItemKind::Result,
                title: "x".into(),
                subtitle: String::new(),
                icon: None,
                score: 1.0,
                source: self.id.into(),
                actions: vec![],
                payload: serde_json::Value::Null,
            }]
        }
        async fn execute(&self, _: &Item, _: Action) -> Result<Outcome, FeatureError> {
            Ok(Outcome::Done)
        }
    }

    fn ctx() -> FeatureCtx {
        FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(Config::default())),
            store: Arc::new(Store::open_in_memory().unwrap()),
            providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        }
    }

    fn input(mode: Mode) -> Input {
        Input {
            raw: "x".into(),
            text: "x".into(),
            mode,
            seq: 1,
        }
    }

    fn stub(id: &'static str, fail: bool, mode: &'static [Mode]) -> Arc<dyn Feature> {
        Arc::new(Stub { id, fail, mode })
    }

    #[tokio::test]
    async fn start_stop_leaves_no_tasks() {
        let r = Registry::new();
        let c = ctx();
        r.register(stub("a", false, &[Mode::Command])).await;
        r.apply_config(&c.config.load(), &c).await;
        assert_eq!(
            r.views(&c.config.load()).await[0].status,
            FeatureState::Running
        );
        assert_eq!(r.on_count().await, (1, 1));
        let scope = r.entries.read().await[0].scope.clone().unwrap();
        assert_eq!(scope.active(), 1);
        let cfg = r.set_enabled("a", false, &c).await.unwrap();
        assert_eq!(cfg.features["a"]["enabled"].as_bool(), Some(false));
        assert_eq!(r.views(&cfg).await[0].status, FeatureState::Stopped);
        assert_eq!(scope.active(), 0);
        assert!(r.query_all(&input(Mode::Command)).is_empty());
        r.set_enabled("a", true, &c).await.unwrap();
        assert_eq!(r.query_all(&input(Mode::Command)).len(), 1);
    }

    #[tokio::test]
    async fn failed_start_is_skipped() {
        let r = Registry::new();
        let c = ctx();
        r.register(stub("bad", true, &[Mode::Command])).await;
        r.apply_config(&c.config.load(), &c).await;
        let v = r.views(&c.config.load()).await;
        assert_eq!(v[0].status, FeatureState::Failed);
        assert!(v[0].error.as_deref().unwrap().contains("boom"));
        assert!(r.query_all(&input(Mode::Command)).is_empty());
    }

    #[tokio::test]
    async fn query_all_filters_by_mode() {
        let r = Registry::new();
        let c = ctx();
        r.register(stub("a", false, &[Mode::Command])).await;
        r.register(stub("b", false, &[Mode::Find])).await;
        r.apply_config(&c.config.load(), &c).await;
        let items = r.query_all(&input(Mode::Find));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "b");
    }

    #[tokio::test]
    async fn set_options_validates_and_restarts() {
        let r = Registry::new();
        let c = ctx();
        r.register(stub("a", false, &[Mode::Command])).await;
        r.apply_config(&c.config.load(), &c).await;
        assert!(
            r.set_options("a", "n = 3".parse().unwrap(), &c)
                .await
                .is_err()
        );
        let cfg = r
            .set_options("a", "n = true".parse().unwrap(), &c)
            .await
            .unwrap();
        assert_eq!(cfg.features["a"]["n"].as_bool(), Some(true));
        assert_eq!(r.views(&cfg).await[0].status, FeatureState::Running);
        assert!(r.set_options("zzz", toml::Table::new(), &c).await.is_err());
    }
}
