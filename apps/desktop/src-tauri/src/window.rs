//! Launcher window lifecycle: prewarmed, shown on demand, hidden on blur.
use std::sync::Mutex;
use std::time::Instant;

use crate::error::CommandError;
use serde_json::json;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow,
    WindowEvent,
};

/// Label of the single launcher window.
pub const LABEL: &str = "launcher";

/// Panel width in logical px. Keep equal to `--l-panel-w` in tokens.css.
const PANEL_W: f64 = 640.0;
/// Tallest panel in logical px. Keep equal to `--l-panel-max` in tokens.css.
const PANEL_MAX_H: f64 = 586.0;
/// Shortest window in logical px (the input row alone).
const PANEL_MIN_H: f64 = 56.0;
/// Gap kept between the panel and the work-area edge, in logical px.
const WORK_MARGIN: f64 = 24.0;

/// Tallest panel (logical px) that fits a work area `work_h` physical px high at `scale`.
fn max_height(work_h: u32, scale: f64) -> f64 {
    (f64::from(work_h) / scale - WORK_MARGIN).clamp(PANEL_MIN_H, PANEL_MAX_H)
}

/// Window origin and width in physical px for a work area `(x, y, w, h)` at `scale`:
/// centred horizontally, top at the vertical centre of a max-height panel.
fn layout(work: (i32, i32, u32, u32), scale: f64) -> (i32, i32, u32) {
    let w = (PANEL_W * scale).round();
    let x = f64::from(work.0) + (f64::from(work.2) - w) / 2.0;
    let tall = max_height(work.3, scale) * scale;
    let y = f64::from(work.1) + ((f64::from(work.3) - tall) / 2.0).max(0.0);
    (x.round() as i32, y.round() as i32, w as u32)
}

fn clamp_height(h: f64) -> f64 {
    if h.is_finite() {
        h.clamp(PANEL_MIN_H, PANEL_MAX_H)
    } else {
        PANEL_MIN_H
    }
}

/// Timing and monitor state shared between toggles and `report_shown`.
#[derive(Default)]
pub struct ShowState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    shown_id: u64,
    t0: Option<Instant>,
    /// Last logical window height set by the UI.
    height: Option<f64>,
    /// Whether the acrylic effect is currently applied.
    glass: bool,
    /// Dark flag last given to the backdrop.
    dark: bool,
}

/// Hooks up blur/close handling for the prewarmed launcher window.
pub fn setup(app: &tauri::App) -> anyhow::Result<()> {
    app.manage(ShowState::default());
    let win = app
        .get_webview_window(LABEL)
        .ok_or_else(|| anyhow::anyhow!("launcher window missing from tauri.conf.json"))?;
    #[cfg(windows)]
    match win.hwnd() {
        Ok(h) => {
            if let Err(err) = win::dwm::keep_caption_off(h.0 as isize) {
                tracing::warn!(%err, "caption guard failed");
            }
            if let Err(err) = win::dwm::round_corners(h.0 as isize) {
                tracing::warn!(%err, "rounded corners failed");
            }
        }
        Err(err) => tracing::warn!(%err, "no window handle"),
    }
    sync_glass(app.handle(), &win);
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
    show_window(app, &win, reason, t0);
}

/// True while the launcher is on screen.
pub fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// Shows the launcher on the monitor under the cursor; unlike `toggle` it never hides.
pub fn show(app: &AppHandle, reason: &str) {
    let Some(win) = app.get_webview_window(LABEL) else {
        tracing::error!("launcher window missing");
        return;
    };
    show_window(app, &win, reason, Instant::now());
}

fn show_window(app: &AppHandle, win: &WebviewWindow, reason: &str, t0: Instant) {
    if let Err(err) = place(app, win) {
        tracing::warn!(%err, "positioning failed");
    }
    if let Err(err) = win.show().and_then(|()| win.set_focus()) {
        tracing::warn!(%err, "show failed");
    }
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        let _ = win::dwm::strip_caption(h.0 as isize);
    }
    let glass = sync_glass(app, win);
    let state: State<ShowState> = app.state();
    let Ok(mut s) = state.inner.lock() else {
        return;
    };
    s.shown_id += 1;
    s.t0 = Some(t0);
    let shown_id = s.shown_id;
    drop(s);
    let config = app
        .state::<std::sync::Arc<crate::state::AppState>>()
        .config
        .load();
    let appearance = crate::commands::settings::AppearanceView::from(&config.appearance);
    emit(
        app,
        "launcher://shown",
        json!({ "shownId": shown_id, "reason": reason, "glass": glass, "appearance": appearance }),
    );
}

/// Applies or removes the acrylic effect to match the Windows transparency setting;
/// returns whether glass is on.
fn sync_glass(app: &AppHandle, win: &WebviewWindow) -> bool {
    #[cfg(windows)]
    let want = win::dwm::transparency_enabled();
    #[cfg(not(windows))]
    let want = false;
    let scheme = app
        .try_state::<std::sync::Arc<crate::state::AppState>>()
        .map(|st| st.config.load().appearance.scheme.clone())
        .unwrap_or_default();
    let dark = is_dark(&scheme);
    let state: State<ShowState> = app.state();
    let Ok(mut s) = state.inner.lock() else {
        return false;
    };
    if s.glass != want || s.dark != dark {
        #[cfg(windows)]
        match win.hwnd() {
            Ok(h) => match win::dwm::set_acrylic(h.0 as isize, want, dark) {
                Ok(()) => {
                    s.glass = want;
                    s.dark = dark;
                }
                Err(err) => tracing::warn!(%err, "acrylic failed"),
            },
            Err(err) => tracing::warn!(%err, "no window handle"),
        }
        #[cfg(not(windows))]
        let _ = win;
    }
    s.glass
}

/// Whether a configured scheme resolves to dark; "system" follows Windows.
fn is_dark(scheme: &str) -> bool {
    match scheme {
        "dark" => true,
        "light" => false,
        #[cfg(windows)]
        _ => win::dwm::system_prefers_dark(),
        #[cfg(not(windows))]
        _ => true,
    }
}

/// Sizes the window to the panel and centres it on the monitor under the cursor.
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
    let (wa, scale) = (m.work_area(), m.scale_factor());
    let (x, y, w) = layout(
        (wa.position.x, wa.position.y, wa.size.width, wa.size.height),
        scale,
    );
    let state: State<ShowState> = app.state();
    let height = state
        .inner
        .lock()
        .map_err(|_| anyhow::anyhow!("state poisoned"))?
        .height
        .unwrap_or(PANEL_MIN_H);
    let pos = PhysicalPosition::new(x, y);
    let height = height.min(max_height(wa.size.height, scale));
    let size = PhysicalSize::new(w, (height * scale).round() as u32);
    // Move first so a cross-monitor DPI change (WM_DPICHANGED) rescales the
    // old size, then set the size in physical px and re-pin the position,
    // since resizing after the move can shift the origin.
    win.set_position(pos)?;
    win.set_size(size)?;
    win.set_position(pos)?;
    Ok(())
}

/// Grows or shrinks the window to the panel's content height (logical px); the top stays put.
#[tauri::command]
pub async fn resize_launcher(
    app: AppHandle,
    state: State<'_, ShowState>,
    height: f64,
) -> Result<(), CommandError> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or_else(|| CommandError::new("window_missing", "launcher window missing"))?;
    let mut h = clamp_height(height);
    if let Ok(Some(m)) = win.current_monitor() {
        h = h.min(max_height(m.work_area().size.height, m.scale_factor()));
    }
    state
        .inner
        .lock()
        .map_err(|_| CommandError::new("internal", "state poisoned"))?
        .height = Some(h);
    win.set_size(LogicalSize::new(PANEL_W, h))
        .map_err(|e| CommandError::new("window", e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_centres_at_scale_1() {
        // 1920x1040 work area with a 40px taskbar at the bottom.
        assert_eq!(layout((0, 0, 1920, 1040), 1.0), (640, 227, 640));
    }

    #[test]
    fn layout_scales_and_offsets() {
        // Second monitor to the right, 150%.
        let (x, y, w) = layout((1920, 0, 2880, 1500), 1.5);
        assert_eq!(w, 960);
        assert_eq!(x, 1920 + (2880 - 960) / 2);
        assert_eq!(y, ((1500.0 - 586.0 * 1.5) / 2.0_f64).round() as i32);
    }

    #[test]
    fn layout_never_above_work_area() {
        let (_, y, _) = layout((0, 10, 800, 300), 1.0);
        assert!(y >= 10, "panel top above the work area: {y}");
        assert_eq!(y, 10 + (WORK_MARGIN / 2.0) as i32);
    }

    #[test]
    fn max_height_fits_short_screens() {
        // 1366x768 at 125%: work area 1366x728 physical -> 582 logical, minus margin.
        assert!((max_height(728, 1.25) - (582.4 - WORK_MARGIN)).abs() < 1e-9);
        assert_eq!(max_height(2000, 1.0), PANEL_MAX_H);
        assert_eq!(max_height(10, 1.0), PANEL_MIN_H);
    }

    #[test]
    fn explicit_schemes_resolve() {
        assert!(is_dark("dark"));
        assert!(!is_dark("light"));
    }

    #[test]
    fn height_is_clamped() {
        assert_eq!(clamp_height(10.0), PANEL_MIN_H);
        assert_eq!(clamp_height(9999.0), PANEL_MAX_H);
        assert_eq!(clamp_height(f64::NAN), PANEL_MIN_H);
        assert_eq!(clamp_height(200.0), 200.0);
    }
}
