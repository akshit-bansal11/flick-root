//! The `Feature` trait (ADR-009).

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, Input, Item, Mode, Outcome};

/// A launcher module with a toggle. Off means stopped: no tasks, watchers, timers or clients.
#[async_trait]
pub trait Feature: Send + Sync {
    /// Stable id, also the `[features.<id>]` key.
    fn id(&self) -> &'static str;
    /// Name on the Features page.
    fn title(&self) -> &'static str;
    /// Version tag such as "v0.0.1".
    fn version_tag(&self) -> &'static str;
    /// One line for the Features page.
    fn description(&self) -> &'static str;
    /// Whether it starts enabled when config has no entry.
    fn default_enabled(&self) -> bool;
    /// Modes in which `query` is called.
    fn modes(&self) -> &'static [Mode];
    /// Options, driving the generated options page.
    fn settings_schema(&self) -> SettingsSchema;
    /// Precompute here; spawn background work only through `ctx.tasks`.
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError>;
    /// Idempotent; drops everything held.
    fn stop(&self);
    /// Sync, cheap, no I/O (budget 0.5 ms).
    fn query(&self, input: &Input) -> Vec<Item>;
    /// Runs an action on an item this feature produced.
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError>;

    /// Extra rank score for any item (for example usage history).
    fn rank_boost(&self, _input: &Input, _item: &Item) -> f32 {
        0.0
    }
    /// Called when the user runs `item` for `input`.
    fn on_chosen(&self, _input: &Input, _item: &Item) {}
    /// True when a changed key is a `restart: true` field.
    fn needs_restart_for(&self, changed_keys: &[String]) -> bool {
        self.settings_schema()
            .fields
            .iter()
            .any(|f| f.restart && changed_keys.contains(&f.key))
    }
}
