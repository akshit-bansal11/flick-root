//! Everything a started feature may use, and the task scope that guarantees stop() leaves nothing running.

use arc_swap::ArcSwap;
use launcher_core::Config;
use launcher_core::store::Store;
use parking_lot::Mutex;
use providers::hub::ProviderHub;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// How long `shutdown` waits for tasks before aborting them.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

/// Receives events for the UI.
pub trait EventSink: Send + Sync {
    /// Emits `name` with `payload`.
    fn emit(&self, name: &str, payload: serde_json::Value);
}

/// Cancellable set of tasks owned by one feature run.
#[derive(Clone)]
pub struct TaskScope {
    token: CancellationToken,
    set: Arc<Mutex<JoinSet<()>>>,
}

impl Default for TaskScope {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskScope {
    /// New empty scope.
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            set: Arc::new(Mutex::new(JoinSet::new())),
        }
    }

    /// Fresh scope cancelled when this one is cancelled. Use once per `start()`.
    pub fn child(&self) -> Self {
        Self {
            token: self.token.child_token(),
            set: Arc::new(Mutex::new(JoinSet::new())),
        }
    }

    /// Token tasks should watch.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Spawns `fut` on the current runtime inside this scope.
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut set = self.set.lock();
        while set.try_join_next().is_some() {}
        set.spawn(fut);
    }

    /// Number of tasks still running.
    pub fn active(&self) -> usize {
        let mut set = self.set.lock();
        while set.try_join_next().is_some() {}
        set.len()
    }

    /// Cancels, waits up to 500 ms, then aborts what is left.
    pub async fn shutdown(&self) {
        self.token.cancel();
        let mut set = std::mem::take(&mut *self.set.lock());
        let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
            while set.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            tracing::warn!(
                left = set.len(),
                "feature tasks missed the deadline, aborting"
            );
            set.abort_all();
            while set.join_next().await.is_some() {}
        }
    }
}

/// Handles given to `Feature::start`. Cheap to clone.
#[derive(Clone)]
pub struct FeatureCtx {
    /// Live config.
    pub config: Arc<ArcSwap<Config>>,
    /// SQLite store.
    pub store: Arc<Store>,
    /// Model providers.
    pub providers: Arc<ArcSwap<ProviderHub>>,
    /// Plain HTTP client (Rust side only; the WebView makes no network calls).
    pub http: reqwest::Client,
    /// Event channel to the UI.
    pub events: Arc<dyn EventSink>,
    /// Task scope of this start.
    pub tasks: TaskScope,
}

impl FeatureCtx {
    /// Copy of `[features.<id>]`, empty when absent.
    pub fn options(&self, id: &str) -> toml::Table {
        self.config
            .load()
            .features
            .get(id)
            .cloned()
            .unwrap_or_default()
    }
}
