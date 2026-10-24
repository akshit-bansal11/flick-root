//! Tauri commands exposed to the UI (flat params).
use crate::error::CommandError;

pub mod ask;
pub mod engines;
pub mod features;
pub mod firstrun;
pub mod home;
pub mod integrations;
pub mod launch;
pub mod providers;
pub mod search;
pub mod secrets;
pub mod settings;
pub mod themes;

/// Runs blocking work (keyring, disk) off the async runtime.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::new("internal", format!("task failed: {e}")))?
}
