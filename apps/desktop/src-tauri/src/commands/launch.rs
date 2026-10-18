//! Launch, reveal, copy path and open link.
use crate::error::CommandError;
use crate::state::AppState;
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

fn app_entry(state: &AppState, parsing: &str) -> Result<index_apps::AppEntry, CommandError> {
    state
        .apps
        .entries()
        .iter()
        .find(|a| a.parsing_name == parsing)
        .cloned()
        .ok_or_else(not_found)
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
            let entry = app_entry(&state, parsing)?;
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
            blocking_com(move || ia::open_path(&p)).await?;
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
            let entry = app_entry(&state, parsing)?;
            blocking_com(move || ia::reveal(&entry)).await?;
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
    fn id_parsing() {
        assert_eq!(parse_id("app:p.X"), IdRef::App("p.X"));
        assert_eq!(parse_id("file:C:\\a b"), IdRef::File("C:\\a b"));
        assert_eq!(parse_id("calc:1"), IdRef::Other);
    }
}
