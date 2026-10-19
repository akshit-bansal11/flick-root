//! Tauri shell for the launcher.
pub mod autostart;
mod commands;
mod error;
mod hotkey;
mod icons;
mod logging;
mod meta_store;
mod state;
mod tray;
mod window;

use tauri::Manager;

/// Builds and runs the Tauri application; logs and exits nonzero on failure.
pub fn run() {
    state::mark_start();
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
            state::phase("setup start");
            window::setup(app)?;
            state::phase("launcher window ready");
            state::init(app)?;
            state::start(app.handle());
            tray::setup(app)?;
            state::phase("tray built");
            // A conflict is already logged and emitted; fall back to the default.
            let saved = app
                .state::<std::sync::Arc<state::AppState>>()
                .config
                .load()
                .general
                .hotkey
                .clone();
            if hotkey::rebind_launcher(app.handle(), &saved).is_err() && saved != hotkey::DEFAULT {
                tracing::warn!(saved, "saved hotkey failed, using default");
                let _ = hotkey::rebind_launcher(app.handle(), hotkey::DEFAULT);
            }
            state::phase("hotkey registered");
            let first_run_done = app
                .state::<std::sync::Arc<state::AppState>>()
                .config
                .load()
                .general
                .first_run_done;
            if !first_run_done {
                // Off the ready path: window creation blocks on the main thread's loop.
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = tray::open_settings_window(&handle, Some("first-run")) {
                        tracing::warn!(%err, "opening first-run failed");
                    }
                    state::phase("first-run window built");
                });
            }
            state::phase("ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::report_shown,
            window::hide_launcher,
            tray::open_settings,
            commands::search::search,
            commands::ask::ask_stream,
            commands::ask::cancel_ask,
            commands::launch::launch,
            commands::launch::reveal,
            commands::launch::copy_path,
            commands::launch::open_link,
            commands::providers::list_providers,
            commands::providers::probe_providers,
            commands::providers::set_provider,
            commands::providers::set_provider_order,
            commands::engines::list_engines,
            commands::engines::probe_decision_engines,
            commands::engines::set_engine,
            commands::secrets::set_secret,
            commands::secrets::delete_secret,
            commands::secrets::test_secret,
            commands::secrets::list_secrets,
            commands::features::list_features,
            commands::features::set_feature_enabled,
            commands::features::set_feature_options,
            commands::features::set_mode_enabled,
            commands::features::run_system_command,
            commands::firstrun::get_first_run,
            commands::firstrun::complete_first_run,
            commands::settings::get_settings,
            commands::settings::set_general,
            commands::settings::set_roots,
            commands::settings::clear_history,
            commands::settings::reindex,
            commands::settings::get_about
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
