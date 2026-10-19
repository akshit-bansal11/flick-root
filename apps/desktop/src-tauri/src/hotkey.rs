//! Global hotkey registration (Rust-side only).
use features::explain;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use std::sync::Mutex;

use crate::error::CommandError;
use crate::window;

/// Default launcher accelerator.
pub const DEFAULT: &str = "Alt+Space";

/// Builds the global-shortcut plugin; any registered shortcut press toggles the launcher.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let is_explain = explain_active()
                .and_then(|a| a.parse::<Shortcut>().ok())
                .is_some_and(|s| s == *shortcut);
            if is_explain {
                crate::explain::on_hotkey(app);
            } else {
                window::toggle(app, "hotkey");
            }
        })
        .build()
}

/// The launcher accelerator this app actually holds (may differ from config after a fallback).
static ACTIVE: Mutex<Option<String>> = Mutex::new(None);

fn active() -> Option<String> {
    ACTIVE.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Rejects accelerators without a modifier (Ctrl/Alt/Shift/Super) so a bare key can never
/// become a system-wide hotkey. Windows-reserved chords are not checked here: the OS refuses
/// them at registration, and the UI explains them first (hotkeyRules.ts).
pub fn validate(accelerator: &str) -> Result<(), CommandError> {
    let mut parts = accelerator.split('+');
    let _key = parts.next_back();
    let has_mod = parts.any(|p| {
        matches!(
            p.trim().to_ascii_lowercase().as_str(),
            "ctrl"
                | "control"
                | "alt"
                | "shift"
                | "super"
                | "win"
                | "meta"
                | "cmd"
                | "commandorcontrol"
                | "cmdorctrl"
        )
    });
    if has_mod {
        Ok(())
    } else {
        Err(CommandError::new(
            "hotkey_invalid",
            "A hotkey needs Ctrl, Alt, Shift or Win plus a key.",
        ))
    }
}

/// Binds the launcher hotkey to `new`, releasing the one this app held before.
/// A no-op when `new` is already ours; on failure the previous binding stays.
pub fn rebind_launcher(app: &AppHandle, new: &str) -> Result<(), CommandError> {
    validate(new)?;
    let prev = active();
    if prev.as_deref() == Some(new) {
        return Ok(());
    }
    register(app, "launcher", new)
        .map_err(|e| CommandError::new("hotkey_conflict", format!("{e:#}")))?;
    if let Some(p) = prev
        && let Err(err) = app.global_shortcut().unregister(p.as_str())
    {
        tracing::warn!(%err, "unregistering old hotkey failed");
    }
    *ACTIVE.lock().unwrap_or_else(|p| p.into_inner()) = Some(new.to_owned());
    Ok(())
}

/// The Explain accelerator this app holds right now; `None` while the feature is off.
static EXPLAIN: Mutex<Option<String>> = Mutex::new(None);

fn explain_active() -> Option<String> {
    EXPLAIN.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Makes the Explain binding match the feature: `accelerator` while the feature runs, none when
/// it is off. A no-op when already right; on failure the previous binding stays.
pub fn sync_explain(app: &AppHandle, accelerator: &str) -> Result<(), CommandError> {
    let want = explain::is_running().then_some(accelerator);
    if let Some(w) = want {
        validate(w)?;
    }
    let prev = explain_active();
    if prev.as_deref() == want {
        return Ok(());
    }
    if let Some(w) = want {
        register(app, "explain", w)
            .map_err(|e| CommandError::new("hotkey_conflict", format!("{e:#}")))?;
    }
    if let Some(p) = prev
        && let Err(err) = app.global_shortcut().unregister(p.as_str())
    {
        tracing::warn!(%err, "unregistering old explain hotkey failed");
    }
    *EXPLAIN.lock().unwrap_or_else(|p| p.into_inner()) = want.map(str::to_owned);
    Ok(())
}

/// Registers `accelerator`; on failure emits `hotkey://conflict` and returns the error.
pub fn register(app: &AppHandle, which: &str, accelerator: &str) -> Result<(), anyhow::Error> {
    app.global_shortcut()
        .register(accelerator)
        .map(|()| {
            tracing::info!(which, accelerator, "hotkey registered");
        })
        .map_err(|err| {
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

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn requires_a_modifier() {
        assert_eq!(validate("Tab").unwrap_err().code, "hotkey_invalid");
        assert_eq!(validate("A").unwrap_err().code, "hotkey_invalid");
        assert_eq!(validate("").unwrap_err().code, "hotkey_invalid");
        assert!(validate("Alt+Space").is_ok());
        assert!(validate("Ctrl+Shift+K").is_ok());
        assert!(validate("Super+L").is_ok());
    }
}
