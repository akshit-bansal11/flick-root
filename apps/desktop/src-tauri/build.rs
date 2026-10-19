fn main() {
    let commands = &[
        "report_shown",
        "hide_launcher",
        "open_settings",
        "search",
        "launch",
        "reveal",
        "copy_path",
        "open_link",
        "run_system_command",
        "ask_stream",
        "cancel_ask",
        "get_settings",
        "set_general",
        "set_roots",
        "clear_history",
        "list_features",
        "set_feature_enabled",
        "set_mode_enabled",
        "set_feature_options",
        "list_providers",
        "probe_providers",
        "set_provider",
        "set_provider_order",
        "list_engines",
        "probe_decision_engines",
        "set_engine",
        "get_integrations",
        "set_integration",
        "set_secret",
        "delete_secret",
        "test_secret",
        "list_secrets",
        "reindex",
        "get_first_run",
        "complete_first_run",
        "get_about",
    ];
    let attrs = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(commands));
    if let Err(e) = tauri_build::try_build(attrs) {
        panic!("tauri build failed: {e:#}");
    }
}
