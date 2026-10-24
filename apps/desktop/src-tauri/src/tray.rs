//! System tray and the Settings window.
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::error::CommandError;
use crate::window;

/// Label of the Settings window.
pub const SETTINGS_LABEL: &str = "settings";

/// Shows the launcher if hidden; never hides it.
pub fn show_launcher(app: &AppHandle, reason: &str) {
    let visible = app
        .get_webview_window(window::LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !visible {
        window::toggle(app, reason);
    }
}

/// Builds the tray icon: left click shows the launcher, right click opens the menu
/// (rendered by the launcher window, see `window::show_menu`).
pub fn setup(app: &tauri::App) -> anyhow::Result<()> {
    crate::update::init(app);
    let mut builder = TrayIconBuilder::new()
        .tooltip(app.package_info().name.clone())
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            match button {
                MouseButton::Left => show_launcher(tray.app_handle(), "tray"),
                MouseButton::Right => window::show_menu(tray.app_handle(), position),
                MouseButton::Middle => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Shows Settings (creating it on first use) and optionally navigates to a section.
pub fn open_settings_window(app: &AppHandle, section: Option<&str>) -> anyhow::Result<()> {
    if let Some(win) = app.get_webview_window(SETTINGS_LABEL) {
        win.show()?;
        win.set_focus()?;
        app.emit_to(
            SETTINGS_LABEL,
            "settings://navigate",
            serde_json::json!({ "section": section }),
        )?;
        return Ok(());
    }
    let url = match section {
        Some(s) => format!("settings.html#{s}"),
        None => "settings.html".to_owned(),
    };
    let win = WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(url.into()))
        .title(format!("{} Settings", app.package_info().name))
        .inner_size(960.0, 640.0)
        .min_inner_size(760.0, 520.0)
        .decorations(true)
        .visible(true)
        .center()
        .build()?;
    sync_settings_theme(app);
    // Hotkeys always come back when Settings goes away, even if the tour did not unmount cleanly.
    let handle = app.clone();
    win.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
            crate::hotkey::set_paused(&handle, false);
        }
        // The OS resets the caption on a system theme change; put Flick's scheme back.
        // Settings also re-resolves a "system" scheme.
        tauri::WindowEvent::ThemeChanged(_) => {
            sync_settings_theme(&handle);
            if let Err(err) = handle.emit(
                "settings://changed",
                serde_json::json!({ "section": "appearance" }),
            ) {
                tracing::warn!(%err, "emit failed");
            }
        }
        _ => {}
    });
    Ok(())
}

/// Puts Flick's dark/light scheme on the native Settings caption, when the window exists.
pub fn sync_settings_theme(app: &AppHandle) {
    #[cfg(windows)]
    if let Some(win) = app.get_webview_window(SETTINGS_LABEL) {
        let dark = settings_dark(app);
        let res = win
            .hwnd()
            .map_err(anyhow::Error::from)
            .and_then(|h| win::dwm::set_dark_frame(h.0 as isize, dark).map_err(Into::into));
        if let Err(err) = res {
            tracing::warn!(%err, "settings theme");
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Whether Settings renders dark under the saved appearance; "system" follows Windows.
#[cfg(windows)]
fn settings_dark(app: &AppHandle) -> bool {
    let (theme, scheme) = app
        .try_state::<std::sync::Arc<crate::state::AppState>>()
        .map(|st| {
            let cfg = st.config.load();
            (cfg.appearance.theme.clone(), cfg.appearance.scheme.clone())
        })
        .unwrap_or_default();
    scheme_dark(&theme, &scheme).unwrap_or_else(win::dwm::system_prefers_dark)
}

/// Dark (`Some(true)`) or light for a theme and scheme, mirroring `FORCED_SCHEME` in theme.ts;
/// `None` means follow the system.
fn scheme_dark(theme: &str, scheme: &str) -> Option<bool> {
    match (theme, scheme) {
        ("frost", _) => Some(false),
        ("midnight" | "aurora" | "ember", _) => Some(true),
        (_, "dark") => Some(true),
        (_, "light") => Some(false),
        _ => None,
    }
}

/// An item of the tray menu, as the launcher UI sends it.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TrayAction {
    Open,
    Settings,
    Update,
    Quit,
}

impl TrayAction {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "settings" => Some(Self::Settings),
            "update" => Some(Self::Update),
            "quit" => Some(Self::Quit),
            _ => None,
        }
    }
}

/// Runs a tray menu item. Accepted only while the launcher window shows the menu.
#[tauri::command]
pub async fn tray_action(app: AppHandle, action: String) -> Result<(), CommandError> {
    let action = TrayAction::parse(&action)
        .ok_or_else(|| CommandError::new("invalid_action", "unknown tray action"))?;
    // Check and clear in one step: a second concurrent call finds the menu already taken.
    if !window::take_menu(&app) {
        return Err(CommandError::new(
            "not_available",
            "the tray menu is not open",
        ));
    }
    match action {
        // Show in place, without a hide first: only `launcher://shown` is sent, and Root hands
        // its payload to the launcher it mounts in place of the menu.
        TrayAction::Open => window::show(&app, "tray"),
        TrayAction::Settings => {
            window::hide_now(&app);
            open_settings_window(&app, None)?;
        }
        TrayAction::Update => {
            window::hide_now(&app);
            crate::update::on_tray(&app);
        }
        // RunEvent::Exit runs the normal shutdown.
        TrayAction::Quit => app.exit(0),
    }
    Ok(())
}

/// Opens Settings on behalf of the launcher UI.
#[tauri::command]
pub async fn open_settings(app: AppHandle, section: Option<String>) -> Result<(), CommandError> {
    open_settings_window(&app, section.as_deref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_actions_parse() {
        assert_eq!(TrayAction::parse("open"), Some(TrayAction::Open));
        assert_eq!(TrayAction::parse("settings"), Some(TrayAction::Settings));
        assert_eq!(TrayAction::parse("update"), Some(TrayAction::Update));
        assert_eq!(TrayAction::parse("quit"), Some(TrayAction::Quit));
        for bad in ["", "Quit", " open", "exit", "settings\0"] {
            assert_eq!(TrayAction::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn scheme_dark_table() {
        assert_eq!(scheme_dark("frost", "dark"), Some(false));
        assert_eq!(scheme_dark("midnight", "light"), Some(true));
        assert_eq!(scheme_dark("aurora", "system"), Some(true));
        assert_eq!(scheme_dark("ember", "light"), Some(true));
        assert_eq!(scheme_dark("glass", "dark"), Some(true));
        assert_eq!(scheme_dark("solid", "light"), Some(false));
        assert_eq!(scheme_dark("glass", "system"), None);
    }
}
