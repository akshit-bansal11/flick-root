//! First-run commands: state for the welcome flow and its completion.
use super::blocking;
use crate::error::CommandError;
use crate::state::AppState;
use crate::tray;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// What the first-run screen needs.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FirstRunView {
    /// First run already completed.
    pub done: bool,
    /// Configured launcher hotkey.
    pub hotkey: String,
    /// Whether that hotkey is currently registered.
    pub hotkey_registered: bool,
}

fn view(done: bool, hotkey: String, hotkey_registered: bool) -> FirstRunView {
    FirstRunView {
        done,
        hotkey,
        hotkey_registered,
    }
}

/// Returns first-run state.
#[tauri::command]
pub async fn get_first_run(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<FirstRunView, CommandError> {
    let c = state.config.load();
    let hotkey = c.general.hotkey.clone();
    let registered = app.global_shortcut().is_registered(hotkey.as_str());
    Ok(view(c.general.first_run_done, hotkey, registered))
}

/// Marks first run done, closes Settings and shows the launcher. Idempotent.
#[tauri::command]
pub async fn complete_first_run(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|c| {
            c.general.first_run_done = true;
            Ok(())
        })
    })
    .await?;
    if let Err(err) = app.emit(
        "settings://changed",
        serde_json::json!({ "section": "general" }),
    ) {
        tracing::warn!(%err, "emit failed");
    }
    if let Some(win) = app.get_webview_window(tray::SETTINGS_LABEL)
        && let Err(err) = win.hide()
    {
        tracing::warn!(%err, "hiding settings failed");
    }
    tray::show_launcher(&app, "first-run");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_camel_case() {
        let v = serde_json::to_value(view(false, "Alt+Space".into(), true)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "done": false, "hotkey": "Alt+Space", "hotkeyRegistered": true })
        );
    }
}
