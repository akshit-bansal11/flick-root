//! Tauri shell for the launcher.

/// Builds and runs the Tauri application; exits nonzero on failure.
pub fn run() {
    if let Err(err) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("error while running the application: {err}");
        std::process::exit(1);
    }
}
