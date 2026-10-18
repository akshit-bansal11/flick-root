//! Inline math and units feature (stub until its W4 card lands).

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, Input, Item, Mode, Outcome};

/// Inline math and units.
pub struct Math;

#[async_trait]
impl Feature for Math {
    fn id(&self) -> &'static str {
        "math"
    }
    fn title(&self) -> &'static str {
        "Inline math and units"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Results while you type in Command mode: math, units, time zones and currency."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
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
