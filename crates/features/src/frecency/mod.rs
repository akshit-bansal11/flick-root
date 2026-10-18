//! Frecency ranking feature (stub until its W4 card lands).

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, Input, Item, Mode, Outcome};

/// Frecency ranking.
pub struct Frecency;

#[async_trait]
impl Feature for Frecency {
    fn id(&self) -> &'static str {
        "frecency"
    }
    fn title(&self) -> &'static str {
        "Frecency ranking"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Ranks by how often and how recently you used something, and learns per typed prefix."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command, Mode::Find]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema::default()
    }
    fn start(&self, _ctx: FeatureCtx) -> Result<(), FeatureError> {
        Ok(())
    }
    fn stop(&self) {}
    fn query(&self, _input: &Input) -> Vec<Item> {
        Vec::new()
    }
    async fn execute(&self, _item: &Item, _action: Action) -> Result<Outcome, FeatureError> {
        Err(FeatureError::Other("not implemented".into()))
    }
}
