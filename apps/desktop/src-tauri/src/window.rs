//! Launcher window lifecycle: prewarmed, shown on demand, hidden on blur.
use std::sync::Mutex;
use std::time::Instant;

use crate::error::CommandError;
use serde_json::json;
use tauri::window::{Color, Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

/// Label of the single launcher window.
pub const LABEL: &str = "launcher";

type Rect = (i32, i32, u32, u32, u64);

/// Timing and monitor state shared between toggles and `report_shown`.
#[derive(Default)]
pub struct ShowState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    shown_id: u64,
    t0: Option<Instant>,
    monitor: Option<Rect>,
}

/// Hooks up blur/close handling for the prewarmed launcher window.
pub fn setup(app: &tauri::App) -> anyhow::Result<()> {
    app.manage(ShowState::default());
    let win = app
        .get_webview_window(LABEL)
        .ok_or_else(|| anyhow::anyhow!("launcher window missing from tauri.conf.json"))?;
    // Applied once; the UI draws its own dark tint if this is unsupported.
    let acrylic = EffectsBuilder::new()
        .effect(Effect::Acrylic)
        .color(Color(0, 0, 0, 140))
        .build();
    if let Err(err) = win.set_effects(acrylic) {
        tracing::warn!(%err, "acrylic effect unavailable; UI tint only");
    }
    let handle = app.handle().clone();
    let w = win.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => hide(&handle, &w),
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(&handle, &w);
        }
        _ => {}
    });
    Ok(())
}

fn hide(app: &AppHandle, win: &WebviewWindow) {
    if let Err(err) = win.hide() {
        tracing::warn!(%err, "hide failed");
    }
    emit(app, "launcher://hidden", json!({}));
}

fn emit(app: &AppHandle, event: &str, payload: serde_json::Value) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(%err, event, "emit failed");
    }
}

/// Shows the launcher on the monitor under the cursor, or hides it if visible.
pub fn toggle(app: &AppHandle, reason: &str) {
    let t0 = Instant::now();
    tracing::debug!(reason, "toggle");
    let Some(win) = app.get_webview_window(LABEL) else {
        tracing::error!("launcher window missing");
        return;
    };
    if win.is_visible().unwrap_or(false) {
        hide(app, &win);
        return;
    }
    if let Err(err) = place(app, &win) {
        tracing::warn!(%err, "positioning failed");
    }
    if let Err(err) = win.show().and_then(|()| win.set_focus()) {
        tracing::warn!(%err, "show failed");
    }
    let state: State<ShowState> = app.state();
    let Ok(mut s) = state.inner.lock() else {
        return;
    };
    s.shown_id += 1;
    s.t0 = Some(t0);
    let shown_id = s.shown_id;
    drop(s);
    emit(
        app,
        "launcher://shown",
        json!({ "shownId": shown_id, "reason": reason }),
    );
}

/// Covers the whole monitor under the cursor (its full physical bounds).
fn place(app: &AppHandle, win: &WebviewWindow) -> anyhow::Result<()> {
    let cursor = app.cursor_position()?;
    let monitors = app.available_monitors()?;
    let found = monitors.iter().find(|m| {
        let (p, s) = (m.position(), m.size());
        cursor.x >= f64::from(p.x)
            && cursor.x < f64::from(p.x) + f64::from(s.width)
            && cursor.y >= f64::from(p.y)
            && cursor.y < f64::from(p.y) + f64::from(s.height)
    });
    let Some(m) = found.or(monitors.first()) else {
        return Ok(());
    };
    let (pos, size, scale) = (*m.position(), *m.size(), m.scale_factor());
    let key: Rect = (pos.x, pos.y, size.width, size.height, scale.to_bits());
    let state: State<ShowState> = app.state();
    {
        let s = state
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("state poisoned"))?;
        if s.monitor == Some(key) {
            return Ok(());
        }
    }
    // Move first so a cross-monitor DPI change (WM_DPICHANGED) rescales the
    // old size, then set the size in physical px and re-pin the position,
    // since resizing after the move can shift the origin.
    win.set_position(pos)?;
    win.set_size(size)?;
    win.set_position(pos)?;
    if let Ok(mut s) = state.inner.lock() {
        s.monitor = Some(key);
    }
    Ok(())
}

/// Called by the UI after paint; logs hotkey-to-paint latency.
#[tauri::command]
pub async fn report_shown(state: State<'_, ShowState>, shown_id: u64) -> Result<(), CommandError> {
    let s = state
        .inner
        .lock()
        .map_err(|_| CommandError::new("internal", "state poisoned"))?;
    if s.shown_id == shown_id
        && let Some(t0) = s.t0
    {
        let hotkey_to_shown_ms = t0.elapsed().as_secs_f64() * 1000.0;
        tracing::info!(hotkey_to_shown_ms, shown_id, "launcher shown");
    }
    Ok(())
}

/// Hides the launcher on behalf of the UI (Esc, after launch).
#[tauri::command]
pub async fn hide_launcher(app: AppHandle) -> Result<(), CommandError> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or_else(|| CommandError::new("window_missing", "launcher window missing"))?;
    hide(&app, &win);
    Ok(())
}
