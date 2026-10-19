//! System tray and the Settings window.
use tauri::menu::{Menu, MenuItem};
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

/// Builds the tray icon with Open, Settings and Quit entries.
pub fn setup(app: &tauri::App) -> anyhow::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip(app.package_info().name.clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_launcher(app, "tray"),
            "settings" => {
                if let Err(err) = open_settings_window(app, None) {
                    tracing::warn!(%err, "open settings failed");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_launcher(tray.app_handle(), "tray");
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

/// Opens Settings on behalf of the launcher UI.
#[tauri::command]
pub async fn open_settings(app: AppHandle, section: Option<String>) -> Result<(), CommandError> {
    open_settings_window(&app, section.as_deref())?;
    Ok(())
}
