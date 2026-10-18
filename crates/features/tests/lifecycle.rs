//! Lifecycle integration tests: off means stopped (ADR-009).

use arc_swap::ArcSwap;
use async_trait::async_trait;
use features::{
    EventSink, Feature, FeatureCtx, FeatureError, FeatureState, Field, FieldKind, Registry,
    SettingsSchema, TaskScope,
};
use launcher_core::store::Store;
use launcher_core::{Action, Config, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use providers::hub::ProviderHub;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

struct Stub {
    id: &'static str,
    modes: &'static [Mode],
    fail: AtomicBool,
    starts: AtomicUsize,
    ticks: Arc<AtomicUsize>,
    client: Mutex<Option<Arc<()>>>,
}

impl Stub {
    fn new(id: &'static str, modes: &'static [Mode]) -> Arc<Self> {
        Arc::new(Self {
            id,
            modes,
            fail: AtomicBool::new(false),
            starts: AtomicUsize::new(0),
            ticks: Arc::new(AtomicUsize::new(0)),
            client: Mutex::new(None),
        })
    }
}

fn field(key: &str, restart: bool) -> Field {
    Field {
        key: key.into(),
        label: key.into(),
        help: String::new(),
        kind: FieldKind::Bool,
        default: serde_json::json!(false),
        restart,
    }
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
        self.modes
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            fields: vec![field("hot", true), field("cold", false)],
        }
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(FeatureError::Start {
                id: self.id,
                reason: "boom".into(),
            });
        }
        *self.client.lock() = Some(Arc::new(()));
        // Ticking task.
        let (t, n) = (ctx.tasks.token(), self.ticks.clone());
        ctx.tasks.spawn(async move {
            loop {
                tokio::select! {
                    () = t.cancelled() => break,
                    () = tokio::time::sleep(Duration::from_millis(5)) => {
                        n.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
        // Timer.
        let (t, n) = (ctx.tasks.token(), self.ticks.clone());
        ctx.tasks.spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_millis(7));
            loop {
                tokio::select! {
                    () = t.cancelled() => break,
                    _ = iv.tick() => {
                        n.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
        Ok(())
    }
    fn stop(&self) {
        self.client.lock().take();
    }
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

async fn setup(s: &Arc<Stub>) -> (Registry, FeatureCtx) {
    let r = Registry::new();
    let c = ctx();
    r.register(s.clone()).await;
    r.apply_config(&c.config.load(), &c).await;
    (r, c)
}

async fn status(r: &Registry, c: &FeatureCtx) -> FeatureState {
    r.views(&c.config.load()).await[0].status
}

#[tokio::test]
async fn off_stops_tasks_timers_and_drops_client() {
    let s = Stub::new("a", &[Mode::Command]);
    let (r, c) = setup(&s).await;
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(s.ticks.load(Ordering::SeqCst) > 0);
    let held = s.client.lock().clone().unwrap();
    assert_eq!(Arc::strong_count(&held), 2);
    r.set_enabled("a", false, &c).await.unwrap();
    // Registry/feature released its handle; only the test's clone remains.
    assert_eq!(Arc::strong_count(&held), 1);
    assert!(s.client.lock().is_none());
    tokio::time::sleep(Duration::from_millis(20)).await;
    let n = s.ticks.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(s.ticks.load(Ordering::SeqCst), n);
}

#[tokio::test]
async fn task_scope_reaches_zero_within_600ms() {
    let s = Stub::new("a", &[Mode::Command]);
    let r = Registry::new();
    let c = ctx();
    let root = c.tasks.clone();
    r.register(s.clone()).await;
    r.apply_config(&c.config.load(), &c).await;
    r.set_enabled("a", false, &c).await.unwrap();
    let ok = tokio::time::timeout(Duration::from_millis(600), async {
        while root.active() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(ok.is_ok());
    assert_eq!(status(&r, &c).await, FeatureState::Stopped);
}

#[tokio::test]
async fn failed_start_then_retry_succeeds() {
    let s = Stub::new("a", &[Mode::Command]);
    s.fail.store(true, Ordering::SeqCst);
    let (r, c) = setup(&s).await;
    let v = r.views(&c.config.load()).await;
    assert_eq!(v[0].status, FeatureState::Failed);
    assert!(v[0].error.as_deref().unwrap().contains("boom"));
    assert!(r.query_all(&input(Mode::Command)).is_empty());
    s.fail.store(false, Ordering::SeqCst);
    r.set_enabled("a", true, &c).await.unwrap();
    let v = r.views(&c.config.load()).await;
    assert_eq!(v[0].status, FeatureState::Running);
    assert!(v[0].error.is_none());
    assert_eq!(r.query_all(&input(Mode::Command)).len(), 1);
}

#[tokio::test]
async fn repeated_toggling_leaves_one_scope() {
    let s = Stub::new("a", &[Mode::Command]);
    let (r, c) = setup(&s).await;
    for _ in 0..3 {
        r.set_enabled("a", false, &c).await.unwrap();
        r.set_enabled("a", true, &c).await.unwrap();
    }
    assert_eq!(s.starts.load(Ordering::SeqCst), 4);
    assert_eq!(r.on_count().await, (1, 1));
    // Two tasks (ticker + timer) belong to the single live scope; no leaks from earlier runs.
    r.stop_all().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let n = s.ticks.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(s.ticks.load(Ordering::SeqCst), n);
}

#[tokio::test]
async fn only_restart_fields_restart() {
    let s = Stub::new("a", &[Mode::Command]);
    let (r, c) = setup(&s).await;
    r.set_options("a", "cold = true".parse().unwrap(), &c)
        .await
        .unwrap();
    assert_eq!(s.starts.load(Ordering::SeqCst), 1);
    r.set_options("a", "cold = true\nhot = true".parse().unwrap(), &c)
        .await
        .unwrap();
    assert_eq!(s.starts.load(Ordering::SeqCst), 2);
    assert_eq!(status(&r, &c).await, FeatureState::Running);
}

#[tokio::test]
async fn query_all_respects_modes() {
    let a = Stub::new("a", &[Mode::Command]);
    let b = Stub::new("b", &[Mode::Find, Mode::Query]);
    let r = Registry::new();
    let c = ctx();
    r.register(a).await;
    r.register(b).await;
    r.apply_config(&c.config.load(), &c).await;
    let src = |m| {
        r.query_all(&input(m))
            .into_iter()
            .map(|i| i.source)
            .collect::<Vec<_>>()
    };
    assert_eq!(src(Mode::Command), ["a"]);
    assert_eq!(src(Mode::Find), ["b"]);
    assert_eq!(src(Mode::Query), ["b"]);
}

#[tokio::test]
async fn disabled_at_startup_never_starts() {
    let s = Stub::new("a", &[Mode::Command]);
    let r = Registry::new();
    let c = ctx();
    r.register(s.clone()).await;
    let mut cfg = Config::default();
    cfg.features
        .entry("a".into())
        .or_default()
        .insert("enabled".into(), toml::Value::Boolean(false));
    r.apply_config(&cfg, &c).await;
    assert_eq!(s.starts.load(Ordering::SeqCst), 0);
    assert_eq!(status(&r, &c).await, FeatureState::Stopped);
    assert!(r.query_all(&input(Mode::Command)).is_empty());
}
