//! Launch-at-login handling (Rust-side only; no UI permission).
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// Enables or disables launch at login.
pub fn set_autostart(app: &AppHandle, enabled: bool) -> anyhow::Result<()> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()?;
    } else {
        manager.disable()?;
    }
    Ok(())
}

/// Reports whether launch at login is enabled.
pub fn is_autostart(app: &AppHandle) -> anyhow::Result<bool> {
    Ok(app.autolaunch().is_enabled()?)
}
