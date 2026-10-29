//! Launcher window lifecycle: prewarmed, shown on demand, hidden on blur.
use std::sync::Mutex;
use std::time::Instant;

use crate::error::CommandError;
use serde_json::json;
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewWindow,
    WindowEvent,
};

/// Label of the single launcher window.
pub const LABEL: &str = "launcher";

/// Panel width in logical px. Keep equal to `--l-panel-w` in tokens.css.
const PANEL_W: f64 = 820.0;
/// Tallest panel in logical px. Keep equal to `--l-panel-max` in tokens.css.
const PANEL_MAX_H: f64 = 540.0;
/// Floor for the panel height in logical px, for tiny work areas.
const PANEL_MIN_H: f64 = 56.0;
/// Gap kept between the panel and the work-area edge, in logical px.
const WORK_MARGIN: f64 = 24.0;
/// Tray menu width in logical px. Keep equal to `--l-menu-w` in tokens.css.
const MENU_W: f64 = 280.0;
/// Tray menu height in logical px. Keep equal to `--l-menu-h` in tokens.css.
const MENU_H: f64 = 187.0;
/// Gap between the tray click point and the menu, in logical px.
const MENU_GAP: f64 = 8.0;

/// Tallest panel (logical px) that fits a work area `work_h` physical px high at `scale`.
fn max_height(work_h: u32, scale: f64) -> f64 {
    (f64::from(work_h) / scale - WORK_MARGIN).clamp(PANEL_MIN_H, PANEL_MAX_H)
}

/// Window size `(w, h)` in physical px for the full panel in a work area `work_h` physical px
/// high at `scale`; the height is capped to what the work area fits.
fn panel_size(work_h: u32, scale: f64) -> (u32, u32) {
    let w = (PANEL_W * scale).round();
    let h = (max_height(work_h, scale) * scale).round();
    (w as u32, h as u32)
}

/// Window origin and size in physical px for the full panel in a work area `(x, y, w, h)` at
/// `scale`: centred in both directions, never above the work area.
fn layout(work: (i32, i32, u32, u32), scale: f64) -> (i32, i32, u32, u32) {
    let (w, h) = panel_size(work.3, scale);
    let x = f64::from(work.0) + (f64::from(work.2) - f64::from(w)) / 2.0;
    let y = f64::from(work.1) + ((f64::from(work.3) - f64::from(h)) / 2.0).max(0.0);
    (x.round() as i32, y.round() as i32, w, h)
}

/// Window origin and size in physical px for the tray menu opened by a click at `anchor`
/// (physical px) on a monitor whose work area is `(x, y, w, h)` at `scale`.
///
/// The taskbar is where the click lands outside the work area: left or right of it puts the
/// menu beside the click, otherwise the menu opens above a click in the lower half (bottom
/// taskbar, or an auto-hidden one) and below a click in the upper half (top taskbar). The
/// result is clamped inside the work area, pinned to its top-left when the area is too small.
fn menu_layout(work: (i32, i32, u32, u32), scale: f64, anchor: (f64, f64)) -> (i32, i32, u32, u32) {
    let (wx, wy) = (f64::from(work.0), f64::from(work.1));
    let (ww, wh) = (f64::from(work.2), f64::from(work.3));
    let (w, h) = ((MENU_W * scale).round(), (MENU_H * scale).round());
    let gap = MENU_GAP * scale;
    let (ax, ay) = anchor;
    let (x, y) = if ax < wx {
        (ax + gap, ay - h / 2.0)
    } else if ax >= wx + ww {
        (ax - w - gap, ay - h / 2.0)
    } else if ay >= wy + wh / 2.0 {
        (ax - w / 2.0, ay - h - gap)
    } else {
        (ax - w / 2.0, ay + gap)
    };
    let x = x.min(wx + ww - w).max(wx);
    let y = y.min(wy + wh - h).max(wy);
    (x.round() as i32, y.round() as i32, w as u32, h as u32)
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
    /// Dark flag last given to the native frame (its Windows 11 border follows it).
    dark: bool,
    /// Whether the window currently shows the tray menu instead of the launcher.
    menu: bool,
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
    sync_frame(app.handle(), &win);
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
    set_menu(app, false);
    if let Err(err) = win.hide() {
        tracing::warn!(%err, "hide failed");
    } else {
        #[cfg(windows)]
        if let Err(err) = win.with_webview(|w| {
            if let Err(err) = win::webview::on_hidden(&w.controller()) {
                tracing::debug!(%err, "webview suspend failed");
            }
        }) {
            tracing::debug!(%err, "webview suspend dispatch failed");
        }
    }
    emit(app, "launcher://hidden", json!({}));
}

/// Hides the launcher window if it exists.
pub fn hide_now(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        hide(app, &win);
    }
}

fn set_menu(app: &AppHandle, on: bool) {
    if let Ok(mut s) = app.state::<ShowState>().inner.lock() {
        s.menu = on;
    }
}

/// True if the launcher window was showing the tray menu, and clears the flag in the same lock,
/// so of two concurrent tray actions only one sees `true`.
pub fn take_menu(app: &AppHandle) -> bool {
    app.state::<ShowState>()
        .inner
        .lock()
        .is_ok_and(|mut s| std::mem::take(&mut s.menu))
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
    set_menu(app, false);
    if let Err(err) = place(app, win) {
        tracing::warn!(%err, "positioning failed");
    }
    reveal(win);
    // Spawned; nothing on the show path waits for the refresh.
    crate::state::on_open(app);
    strip_caption(win);
    sync_frame(app, win);
    let state: State<ShowState> = app.state();
    let Ok(mut s) = state.inner.lock() else {
        return;
    };
    s.shown_id += 1;
    s.t0 = Some(t0);
    let shown_id = s.shown_id;
    drop(s);
    let config = config(app);
    let appearance = crate::commands::settings::AppearanceView::of(
        &app.state::<std::sync::Arc<crate::state::AppState>>(),
    );
    let running = app
        .state::<std::sync::Arc<crate::state::AppState>>()
        .registry
        .running_ids();
    let sources = live_sources(&config, &running);
    let web_title = running
        .contains(&"web")
        .then(features::web::fallback_title)
        .flatten();
    emit(
        app,
        "launcher://shown",
        json!({
            "shownId": shown_id,
            "reason": reason,
            "appearance": appearance,
            "sources": sources,
            "webTitle": web_title,
        }),
    );
}

/// Resumes the WebView, then shows and focuses the window.
fn reveal(win: &WebviewWindow) {
    resume(win);
    show_focused(win);
}

/// Resumes a suspended WebView. Call before showing so the first frame paints from a live page.
fn resume(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Err(err) = win.with_webview(|w| {
        if let Err(err) = win::webview::on_shown(&w.controller()) {
            tracing::debug!(%err, "webview resume failed");
        }
    }) {
        tracing::debug!(%err, "webview resume dispatch failed");
    }
    #[cfg(not(windows))]
    let _ = win;
}

fn show_focused(win: &WebviewWindow) {
    if let Err(err) = win.show().and_then(|()| win.set_focus()) {
        tracing::warn!(%err, "show failed");
    }
}

fn strip_caption(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        let _ = win::dwm::strip_caption(h.0 as isize);
    }
    #[cfg(not(windows))]
    let _ = win;
}

/// Current config snapshot, shared by the `launcher://shown` and `tray://menu` payloads.
fn config(app: &AppHandle) -> std::sync::Arc<launcher_core::Config> {
    app.state::<std::sync::Arc<crate::state::AppState>>()
        .config
        .load_full()
}

/// Turns the launcher window into the tray menu: placed at the click `anchor` (physical px)
/// on the monitor that contains it, `tray://menu` emitted, then shown and focused (in that order,
/// so the launcher's last screen never flashes before the menu renders).
/// Focus loss hides it like the launcher; the next normal show restores the launcher layout.
pub fn show_menu(app: &AppHandle, anchor: PhysicalPosition<f64>) {
    let Some(win) = app.get_webview_window(LABEL) else {
        tracing::error!("launcher window missing");
        return;
    };
    set_menu(app, true);
    match monitor_at(app, anchor.x, anchor.y) {
        Ok(Some(m)) => {
            if let Err(err) = place_menu(&win, &m, anchor) {
                tracing::warn!(%err, "menu positioning failed");
            }
        }
        Ok(None) => {}
        Err(err) => tracing::warn!(%err, "monitor lookup failed"),
    }
    // Resume first so the page receives the event before the window appears.
    resume(&win);
    sync_frame(app, &win);
    let config = config(app);
    let appearance = crate::commands::settings::AppearanceView::of(
        &app.state::<std::sync::Arc<crate::state::AppState>>(),
    );
    let update = crate::update::available(app).map(|version| json!({ "version": version }));
    emit(
        app,
        "tray://menu",
        json!({
            "appVersion": env!("CARGO_PKG_VERSION"),
            "hotkey": config.general.hotkey,
            "update": update,
            "appearance": appearance,
        }),
    );
    show_focused(&win);
    strip_caption(&win);
}

/// Sizes and moves the window to the menu layout for `anchor` in `m`'s work area.
fn place_menu(
    win: &WebviewWindow,
    m: &Monitor,
    anchor: PhysicalPosition<f64>,
) -> tauri::Result<()> {
    let (wa, scale) = (m.work_area(), m.scale_factor());
    let (x, y, w, h) = menu_layout(
        (wa.position.x, wa.position.y, wa.size.width, wa.size.height),
        scale,
        (anchor.x, anchor.y),
    );
    set_bounds(win, PhysicalPosition::new(x, y), PhysicalSize::new(w, h))
}

/// Source ids the launcher can offer now: `apps` always, `files` while Find is on, `ask` while
/// Query is on, then every running feature id. The UI shows a prefix only while one of its
/// sources is live.
fn live_sources(cfg: &launcher_core::Config, running: &[&'static str]) -> Vec<String> {
    let mut v = vec!["apps".to_string()];
    if cfg.modes.find.enabled {
        v.push("files".into());
    }
    if cfg.modes.query.enabled {
        v.push("ask".into());
    }
    v.extend(running.iter().map(|id| (*id).to_string()));
    v
}

/// Puts the configured dark/light scheme on the native frame (the Windows 11 border colour
/// follows it); skipped while the scheme is unchanged.
fn sync_frame(app: &AppHandle, win: &WebviewWindow) {
    let scheme = app
        .try_state::<std::sync::Arc<crate::state::AppState>>()
        .map(|st| st.config.load().appearance.scheme.clone())
        .unwrap_or_default();
    let dark = crate::commands::themes::scheme_is_dark(&scheme);
    let state: State<ShowState> = app.state();
    let Ok(mut s) = state.inner.lock() else {
        return;
    };
    if s.dark != dark {
        #[cfg(windows)]
        match win.hwnd() {
            Ok(h) => match win::dwm::set_dark_frame(h.0 as isize, dark) {
                Ok(()) => s.dark = dark,
                Err(err) => tracing::warn!(%err, "dark frame failed"),
            },
            Err(err) => tracing::warn!(%err, "no window handle"),
        }
        #[cfg(not(windows))]
        let _ = win;
    }
}

/// Sizes the window to the full panel and places it on the monitor under the cursor.
/// The size is fixed for this show: the panel never resizes while open, so the window cannot
/// trail the content.
fn place(app: &AppHandle, win: &WebviewWindow) -> anyhow::Result<()> {
    let cursor = app.cursor_position()?;
    let Some(m) = monitor_at(app, cursor.x, cursor.y)? else {
        return Ok(());
    };
    place_panel(win, &m)?;
    Ok(())
}

/// The monitor containing the physical point `(x, y)`, else the first monitor.
fn monitor_at(app: &AppHandle, x: f64, y: f64) -> tauri::Result<Option<Monitor>> {
    let mut monitors = app.available_monitors()?;
    let found = monitors.iter().position(|m| {
        let (p, s) = (m.position(), m.size());
        x >= f64::from(p.x)
            && x < f64::from(p.x) + f64::from(s.width)
            && y >= f64::from(p.y)
            && y < f64::from(p.y) + f64::from(s.height)
    });
    let i = found.unwrap_or(0);
    Ok((i < monitors.len()).then(|| monitors.swap_remove(i)))
}

/// Sizes the window to the full-height panel (capped to `m`'s work area), centred in it.
fn place_panel(win: &WebviewWindow, m: &Monitor) -> tauri::Result<()> {
    let (wa, scale) = (m.work_area(), m.scale_factor());
    let (x, y, w, h) = layout(
        (wa.position.x, wa.position.y, wa.size.width, wa.size.height),
        scale,
    );
    set_bounds(win, PhysicalPosition::new(x, y), PhysicalSize::new(w, h))
}

fn set_bounds(
    win: &WebviewWindow,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> tauri::Result<()> {
    // Move first so a cross-monitor DPI change (WM_DPICHANGED) rescales the
    // old size, then set the size in physical px and re-pin the position,
    // since resizing after the move can shift the origin.
    win.set_position(pos)?;
    win.set_size(size)?;
    win.set_position(pos)
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
        let (w, h) = (PANEL_W as u32, PANEL_MAX_H as u32);
        assert_eq!(
            layout((0, 0, 1920, 1040), 1.0),
            (((1920 - w) / 2) as i32, ((1040 - h) / 2) as i32, w, h)
        );
    }

    #[test]
    fn layout_scales_and_offsets() {
        // Second monitor to the right, 150%.
        let (x, y, w, h) = layout((1920, 0, 2880, 1500), 1.5);
        let pw = (PANEL_W * 1.5) as u32;
        assert_eq!((w, h), (pw, 810));
        assert_eq!(x, 1920 + ((2880 - pw) / 2) as i32);
        assert_eq!(y, (1500 - 810) / 2);
    }

    #[test]
    fn panel_size_caps_to_the_work_area() {
        let (_, h) = panel_size(680, 1.5);
        assert_eq!(h, ((680.0 / 1.5 - WORK_MARGIN) * 1.5_f64).round() as u32);
    }

    #[test]
    fn panel_size_mirrors_tokens_css() {
        // tokens.css --l-panel-w / --l-panel-max mirror these.
        assert_eq!(PANEL_W, 820.0);
        assert_eq!(PANEL_MAX_H, 540.0);
    }

    #[test]
    fn layout_never_above_work_area() {
        let (_, y, _, _) = layout((0, 10, 800, 300), 1.0);
        assert!(y >= 10, "panel top above the work area: {y}");
        assert_eq!(y, 10 + (WORK_MARGIN / 2.0) as i32);
    }

    #[test]
    fn max_height_fits_short_screens() {
        // 1366x768 at 125%: work area 728 physical -> 582.4 logical; the full panel fits.
        assert_eq!(max_height(728, 1.25), PANEL_MAX_H);
        // 1280x720 at 150%: work area 680 physical -> 453.3 logical, minus margin, so it caps.
        assert!((max_height(680, 1.5) - (680.0 / 1.5 - WORK_MARGIN)).abs() < 1e-9);
        assert!(max_height(680, 1.5) < PANEL_MAX_H);
        assert_eq!(max_height(2000, 1.0), PANEL_MAX_H);
        assert_eq!(max_height(10, 1.0), PANEL_MIN_H);
    }

    #[test]
    fn live_sources_follow_modes_and_running_features() {
        let mut cfg = launcher_core::Config::default();
        let run = ["math", "syscmd"];
        assert_eq!(
            live_sources(&cfg, &run),
            ["apps", "files", "ask", "math", "syscmd"]
        );
        cfg.modes.find.enabled = false;
        assert_eq!(live_sources(&cfg, &run), ["apps", "ask", "math", "syscmd"]);
        cfg.modes.find.enabled = true;
        cfg.modes.query.enabled = false;
        assert_eq!(
            live_sources(&cfg, &run),
            ["apps", "files", "math", "syscmd"]
        );
        assert_eq!(live_sources(&cfg, &[]), ["apps", "files"]);
    }

    #[test]
    fn explicit_schemes_resolve() {
        use crate::commands::themes::scheme_is_dark;
        assert!(scheme_is_dark("dark"));
        assert!(!scheme_is_dark("light"));
    }

    // Menu is 280x187 with an 8 px gap at scale 1.

    #[test]
    fn menu_above_a_bottom_taskbar() {
        // Work area 1920x1040, click on the taskbar at y 1060: above, clamped to the work area.
        assert_eq!(
            menu_layout((0, 0, 1920, 1040), 1.0, (1700.0, 1060.0)),
            (1560, 853, 280, 187)
        );
    }

    #[test]
    fn menu_above_an_auto_hidden_taskbar() {
        // Work area is the whole monitor: bottom edge sits one gap above the click.
        let (_, y, _, h) = menu_layout((0, 0, 1920, 1080), 1.0, (960.0, 1000.0));
        assert_eq!(y + h as i32, 1000 - 8);
    }

    #[test]
    fn menu_below_a_top_taskbar() {
        // Work area starts at y 40; click at y 20.
        assert_eq!(
            menu_layout((0, 40, 1920, 1040), 1.0, (1700.0, 20.0)),
            (1560, 40, 280, 187)
        );
        // A click in the upper half (auto-hidden top taskbar) opens below with the gap.
        let (_, y, _, _) = menu_layout((0, 0, 1920, 1080), 1.0, (960.0, 100.0));
        assert_eq!(y, 108);
    }

    #[test]
    fn menu_beside_a_left_taskbar() {
        // Work area starts at x 48; click at x 24, y 900.
        assert_eq!(
            menu_layout((48, 0, 1872, 1080), 1.0, (24.0, 900.0)),
            (48, 807, 280, 187)
        );
    }

    #[test]
    fn menu_beside_a_right_taskbar() {
        // Work area 0..1872; click at x 1896, y 1070: left of the click, clamped to the bottom.
        assert_eq!(
            menu_layout((0, 0, 1872, 1080), 1.0, (1896.0, 1070.0)),
            (1592, 893, 280, 187)
        );
    }

    #[test]
    fn menu_clamps_to_the_edges() {
        // Far right of a bottom taskbar: right edge stays in the work area.
        let (x, _, w, _) = menu_layout((0, 0, 1920, 1040), 1.0, (1910.0, 1060.0));
        assert_eq!(x + w as i32, 1920);
        // Far left: left edge at the work-area origin.
        let (x, _, _, _) = menu_layout((0, 0, 1920, 1040), 1.0, (5.0, 1060.0));
        assert_eq!(x, 0);
    }

    #[test]
    fn menu_scales_on_an_offset_monitor() {
        // Monitor left of the primary at 150%: 2880x1560 work area at x -2880.
        let (x, y, w, h) = menu_layout((-2880, 0, 2880, 1560), 1.5, (-1440.0, 1590.0));
        assert_eq!((w, h), (420, 281));
        assert_eq!(x, -1440 - 210);
        assert_eq!(y, 1560 - 281);
    }

    #[test]
    fn menu_in_a_tiny_work_area_pins_to_the_origin() {
        assert_eq!(
            menu_layout((100, 50, 200, 100), 1.0, (150.0, 160.0)),
            (100, 50, 280, 187)
        );
    }
}
