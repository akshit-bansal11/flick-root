//! Global hotkey registration (Rust-side only).
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::window;

/// Default launcher accelerator.
pub const DEFAULT: &str = "Alt+Space";

/// Builds the global-shortcut plugin; any registered shortcut press toggles the launcher.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                window::toggle(app, "hotkey");
            }
        })
        .build()
}

/// Registers `accelerator`; on failure emits `hotkey://conflict` and returns the error.
pub fn register(app: &AppHandle, which: &str, accelerator: &str) -> Result<(), anyhow::Error> {
    app.global_shortcut().register(accelerator).map_err(|err| {
        tracing::warn!(%err, which, accelerator, "hotkey registration failed");
        if let Err(e) = app.emit(
            "hotkey://conflict",
            json!({ "which": which, "accelerator": accelerator }),
        ) {
            tracing::warn!(%e, "emit failed");
        }
        anyhow::Error::new(err).context(format!("registering {which} hotkey {accelerator}"))
    })
}
