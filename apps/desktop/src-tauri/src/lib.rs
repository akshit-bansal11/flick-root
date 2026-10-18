//! Tauri shell for the launcher.
pub mod autostart;
mod commands;
mod error;
mod hotkey;
mod icons;
mod logging;
mod state;
mod tray;
mod window;

/// Builds and runs the Tauri application; logs and exits nonzero on failure.
pub fn run() {
    logging::install_panic_hook();
    let log_dir = launcher_core::config::local_data_dir()
        .ok()
        .map(|d| d.join("logs"));
    // Held for the app's lifetime; flushes on drop.
    let _log_guard = match logging::init(log_dir) {
        Ok(g) => Some(g),
        Err(err) => {
            eprintln!("logging init failed: {err}");
            None
        }
    };
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_launcher(app, "second-instance");
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(hotkey::plugin())
        .setup(|app| {
            window::setup(app)?;
            state::init(app)?;
            state::start(app.handle());
            tray::setup(app)?;
            // A conflict is already logged and emitted; keep running.
            let _ = hotkey::register(app.handle(), "launcher", hotkey::DEFAULT);
            tracing::info!("ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::report_shown,
            window::hide_launcher,
            tray::open_settings,
            commands::search::search,
            commands::launch::launch,
            commands::launch::reveal,
            commands::launch::copy_path,
            commands::launch::open_link
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                state::shutdown(handle);
            }
        }),
        Err(err) => {
            tracing::error!(%err, "error while running the application");
            std::process::exit(1);
        }
    }
}
