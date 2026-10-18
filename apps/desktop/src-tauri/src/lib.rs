//! Tauri shell for the launcher.
pub mod autostart;
mod error;
mod hotkey;
mod tray;
mod window;

use tracing_subscriber::EnvFilter;

/// Builds and runs the Tauri application; logs and exits nonzero on failure.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_launcher(app, "second-instance");
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(hotkey::plugin())
        .setup(|app| {
            window::setup(app)?;
            tray::setup(app)?;
            // A conflict is already logged and emitted; keep running.
            let _ = hotkey::register(app.handle(), "launcher", hotkey::DEFAULT);
            tracing::info!("ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::report_shown,
            window::hide_launcher,
            tray::open_settings
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(%err, "error while running the application");
        std::process::exit(1);
    }
}
