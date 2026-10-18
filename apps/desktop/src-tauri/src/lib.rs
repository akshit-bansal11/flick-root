//! Tauri shell for the launcher.
mod autostart;
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
        .plugin(hotkey::plugin())
        .setup(|app| {
            window::setup(app)?;
            // A conflict is already logged and emitted; keep running.
            let _ = hotkey::register(app.handle(), "launcher", hotkey::DEFAULT);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::report_shown,
            window::hide_launcher
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(%err, "error while running the application");
        std::process::exit(1);
    }
}
