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

/// Builds the tray icon: left click shows the launcher, right click opens the glass menu
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
        None => "settings.html".to_string(),
    };
    let win = WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(url.into()))
        .title(format!("{} Settings", app.package_info().name))
        .inner_size(960.0, 640.0)
        .min_inner_size(760.0, 520.0)
        .decorations(true)
        .visible(true)
        .center()
        .build()?;
    // Hotkeys always come back when Settings goes away, even if the tour did not unmount cleanly.
    let handle = app.clone();
    win.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            crate::hotkey::set_paused(&handle, false);
        }
    });
    Ok(())
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
    if !window::menu_active(&app) {
        return Err(CommandError::new(
            "not_available",
            "the tray menu is not open",
        ));
    }
    match action {
        TrayAction::Open => {
            window::hide_now(&app);
            show_launcher(&app, "tray");
        }
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
}
