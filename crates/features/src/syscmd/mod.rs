//! System commands feature (stub until its W4 card lands).

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, Input, Item, Mode, Outcome};

/// System commands.
pub struct SysCmd;

#[async_trait]
impl Feature for SysCmd {
    fn id(&self) -> &'static str {
        "syscmd"
    }
    fn title(&self) -> &'static str {
        "System commands"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Lock, sleep, restart, shut down, Wi-Fi and Bluetooth toggles and more."
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
