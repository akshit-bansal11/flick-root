//! Launch, reveal, copy path and open link.
use crate::error::CommandError;
use crate::state::AppState;
use features::search::program_entry;
use index_apps::AppsError;
use index_apps::launch as ia;
use launcher_core::{Action, ActionKind, Item, Outcome};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// What an item id points at.
#[derive(Debug, PartialEq, Eq)]
enum IdRef<'a> {
    App(&'a str),
    File(&'a str),
    Other,
}

fn parse_id(id: &str) -> IdRef<'_> {
    if let Some(p) = id.strip_prefix("app:") {
        IdRef::App(p)
    } else if let Some(p) = id.strip_prefix("file:") {
        IdRef::File(p)
    } else {
        IdRef::Other
    }
}

/// Accepts only absolute http and https URLs.
fn check_link(url: &str) -> Result<url::Url, CommandError> {
    let u =
        url::Url::parse(url).map_err(|_| CommandError::new("invalid_url", "not a valid URL"))?;
    match u.scheme() {
        "http" | "https" => Ok(u),
        _ => Err(CommandError::new(
            "invalid_url",
            "only http and https links open",
        )),
    }
}

fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("internal", e.to_string())
}

fn not_found() -> CommandError {
    CommandError::new("not_found", "that result is no longer available")
}

fn item(state: &AppState, id: &str) -> Result<Item, CommandError> {
    state
        .last_results
        .lock()
        .get(id)
        .cloned()
        .ok_or_else(not_found)
}

/// The app entry for a parsing name and whether it is a scanned program (not a real app).
fn app_entry(
    state: &AppState,
    parsing: &str,
) -> Result<(index_apps::AppEntry, bool), CommandError> {
    let find = |l: &[index_apps::AppEntry]| l.iter().find(|a| a.parsing_name == parsing).cloned();
    if let Some(e) = find(&state.app_match.load().entries) {
        return Ok((e, false));
    }
    let set = state.programs.load();
    set.paths
        .iter()
        .any(|p| &**p == parsing)
        .then(|| (program_entry(parsing), true))
        .ok_or_else(not_found)
}

/// True when `parsing` is a scanned program on a removable drive, or is no longer in the current
/// program list (the set can be swapped between lookups). Fails closed: checked here, not only in
/// the UI, so a removable or vanished program never runs elevated, even if the IPC call asks.
fn is_removable_program(state: &AppState, parsing: &str) -> bool {
    let set = state.programs.load();
    set.paths
        .iter()
        .position(|p| &**p == parsing)
        .and_then(|i| set.removable.get(i).copied())
        .unwrap_or(true)
}

/// Ok when `path` still exists; else the `not_found` error shown to the user. Blocking: an
/// index entry can sit on a detached or sleeping drive, so call it from a blocking thread.
fn check_exists(path: &Path) -> Result<(), CommandError> {
    if path.exists() {
        Ok(())
    } else {
        Err(CommandError::new(
            "not_found",
            "That file no longer exists (moved, deleted or on a detached drive).",
        ))
    }
}

/// [`check_exists`] on the blocking pool.
async fn ensure_exists(path: PathBuf) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || check_exists(&path))
        .await
        .map_err(internal)?
}

/// Runs `f` on a blocking thread with COM initialised. A dismissed UAC prompt maps to `Ok(false)`.
async fn blocking_com<F>(f: F) -> Result<bool, CommandError>
where
    F: FnOnce() -> Result<(), AppsError> + Send + 'static,
{
    let r = tauri::async_runtime::spawn_blocking(move || {
        let _com = index_apps::com::ComGuard::new().map_err(AppsError::Enumerate)?;
        f()
    })
    .await
    .map_err(internal)?;
    match r {
        Ok(()) => Ok(true),
        Err(AppsError::Cancelled) => Ok(false),
        Err(e) => Err(CommandError::new("launch_failed", format!("{e:#}"))),
    }
}

fn path_of(it: &Item) -> Option<PathBuf> {
    match parse_id(&it.id.0) {
        IdRef::File(p) => Some(PathBuf::from(p)),
        _ => None,
    }
}

/// The text to copy when the item's action is `CopyResult` (`payload.value`).
fn copy_value(it: &Item) -> Option<String> {
    if !it.actions.contains(&ActionKind::CopyResult) {
        return None;
    }
    it.payload.get("value")?.as_str().map(str::to_owned)
}

/// Launches or opens a result and hides the launcher when appropriate.
#[tauri::command]
pub async fn launch(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    item_id: String,
    admin: bool,
) -> Result<Outcome, CommandError> {
    let it = item(&state, &item_id)?;
    let t0 = Instant::now();
    let outcome = match parse_id(&item_id) {
        IdRef::App(parsing) => {
            let (entry, program) = app_entry(&state, parsing)?;
            if program {
                ensure_exists(PathBuf::from(parsing)).await?;
            }
            if admin && !features::search::can_run_as_admin(&entry, program) {
                return Err(CommandError::new(
                    "unsupported",
                    "only .exe and .msi files can run as administrator",
                ));
            }
            if admin && program && is_removable_program(&state, parsing) {
                return Err(CommandError::new(
                    "unsupported",
                    "programs on removable drives cannot run as administrator",
                ));
            }
            let how = if admin {
                ia::LaunchKind::Admin
            } else {
                ia::LaunchKind::Normal
            };
            if blocking_com(move || ia::launch(&entry, how)).await? {
                Outcome::HideLauncher
            } else {
                Outcome::Done // UAC dismissed: stay open
            }
        }
        IdRef::File(p) => {
            let p = PathBuf::from(p);
            ensure_exists(p.clone()).await?;
            blocking_com(move || ia::open_path(&p)).await?;
            Outcome::HideLauncher
        }
        IdRef::Other if it.payload.get("open_settings").is_some() => {
            // A feature row that points at a Settings section (for example "Set up quick capture").
            let section = it.payload["open_settings"].as_str();
            crate::tray::open_settings_window(&app, section).map_err(internal)?;
            Outcome::HideLauncher
        }
        IdRef::Other if copy_value(&it).is_some() => {
            let value = copy_value(&it).unwrap_or_default();
            app.clipboard().write_text(value).map_err(internal)?;
            Outcome::Copied
        }
        IdRef::Other => {
            let f = state.registry.get(&it.source).await.ok_or_else(not_found)?;
            let kind = if admin {
                ActionKind::Admin
            } else {
                ActionKind::Primary
            };
            let action = Action {
                kind,
                confirm_token: None,
            };
            f.execute(&it, action)
                .await
                .map_err(|e| CommandError::new("feature_failed", e.to_string()))?
        }
    };
    if let Outcome::ShowItems { items } = &outcome {
        // The list replaces the results; later Enter/reveal/copy look items up here.
        *state.last_results.lock() = items.iter().map(|i| (i.id.0.clone(), i.clone())).collect();
    }
    tracing::debug!(ms = t0.elapsed().as_millis() as u64, "launch call");
    if outcome == Outcome::HideLauncher {
        record(&state, &it);
        crate::window::hide_launcher(app).await?;
    }
    Ok(outcome)
}

/// Frecency (via `on_chosen`) owns history writes; the shell only notifies features.
fn record(state: &AppState, it: &Item) {
    if let Some(input) = state.last_input.lock().clone() {
        state.registry.on_chosen_all(&input, it);
    }
}

/// Shows a result in Explorer.
#[tauri::command]
pub async fn reveal(state: State<'_, Arc<AppState>>, item_id: String) -> Result<(), CommandError> {
    let it = item(&state, &item_id)?;
    match parse_id(&item_id) {
        IdRef::App(parsing) => {
            let (entry, program) = app_entry(&state, parsing)?;
            // Program entries carry no target; their parsing name is the file path.
            blocking_com(move || {
                if program {
                    ia::reveal_path(Path::new(&entry.parsing_name))
                } else {
                    ia::reveal(&entry)
                }
            })
            .await?;
        }
        IdRef::File(_) => {
            let p = path_of(&it).ok_or_else(not_found)?;
            blocking_com(move || ia::reveal_path(&p)).await?;
        }
        IdRef::Other => return Err(CommandError::new("unsupported", "nothing to reveal")),
    }
    Ok(())
}

/// Copies the path of a Find result to the clipboard.
#[tauri::command]
pub async fn copy_path(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    item_id: String,
) -> Result<(), CommandError> {
    let it = item(&state, &item_id)?;
    let p = path_of(&it).ok_or_else(|| CommandError::new("unsupported", "no path to copy"))?;
    app.clipboard()
        .write_text(p.to_string_lossy().into_owned())
        .map_err(internal)
}

/// Copies text the user already sees (an answer or result value). Written with the history-exclusion
/// formats, so Windows clipboard history and cloud sync skip it.
#[tauri::command]
pub async fn copy_text(text: String) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || win::clipboard::write_text(&text))
        .await
        .map_err(internal)?
        .map_err(internal)
}

/// Opens an http(s) link in the default browser. Only called on user click.
#[tauri::command]
pub async fn open_link(url: String) -> Result<(), CommandError> {
    let u = check_link(&url)?;
    blocking_com(move || ia::open_path(Path::new(u.as_str()))).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_schemes() {
        assert!(check_link("https://example.com/a").is_ok());
        assert!(check_link("http://example.com").is_ok());
        for bad in [
            "file:///c:/x",
            "javascript:alert(1)",
            "ms-settings:",
            "nope",
            "calc.exe",
        ] {
            assert_eq!(check_link(bad).unwrap_err().code, "invalid_url", "{bad}");
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = std::env::temp_dir().join("flick-launch-test-exists");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(check_exists(&dir).is_ok());
        let err = check_exists(&dir.join("gone.exe")).unwrap_err();
        assert_eq!(err.code, "not_found");
        assert!(err.message.contains("no longer exists"));
    }

    #[test]
    fn id_parsing() {
        assert_eq!(parse_id("app:p.X"), IdRef::App("p.X"));
        assert_eq!(parse_id("file:C:\\a b"), IdRef::File("C:\\a b"));
        assert_eq!(parse_id("calc:1"), IdRef::Other);
    }
}
