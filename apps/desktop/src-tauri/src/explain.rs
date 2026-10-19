//! Explain this (F09): the second hotkey captures the selected text and opens Query mode with it.
//! Nothing is sent here; the UI shows the text and the user presses Enter (the ask path re-checks it).
use crate::state::AppState;
use crate::window;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Handles a press of the Explain hotkey.
pub fn on_hotkey(app: &AppHandle) {
    // The launcher already has focus, so there is no foreground selection to read.
    if window::is_visible(app) {
        return;
    }
    let query_on = app
        .state::<Arc<AppState>>()
        .config
        .load()
        .modes
        .query
        .enabled;
    if !query_on {
        tracing::info!("explain skipped: Query mode is off");
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // COM, UI Automation, the clipboard and sleeps: off the async threads.
        let captured = tokio::task::spawn_blocking(|| {
            let text = win::uia::selected_text().unwrap_or_else(|err| {
                tracing::warn!(%err, "reading the selection failed");
                None
            });
            let source = win::foreground::foreground_app()
                .ok()
                .flatten()
                .unwrap_or_default();
            features::explain::analyze(text.as_deref(), &source)
        })
        .await;
        let Ok(captured) = captured else {
            tracing::warn!("explain capture task failed");
            return;
        };
        window::show(&app, "explain");
        if let Err(err) = app.emit("explain://captured", &captured) {
            tracing::warn!(%err, "emit failed");
        }
    });
}
