//! General settings, history, reindex, about and window sizing commands (BSD section 4).
use super::blocking;
use crate::autostart;
use crate::error::CommandError;
use crate::hotkey;
use crate::state::{self, AppState};
use launcher_core::brand::APP_NAME;
use launcher_core::store::ClearScope;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Settings the UI shows. No secret is ever included.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Launcher hotkey.
    pub hotkey_launcher: String,
    /// Explain hotkey.
    pub hotkey_explain: String,
    /// `system`, `dark` or `light`.
    pub theme: String,
    /// Launch at login, as the OS reports it.
    pub launch_at_startup: bool,
    /// Find mode on.
    pub find_enabled: bool,
    /// Query mode on.
    pub query_enabled: bool,
    /// Known-folder roots.
    pub roots: Vec<String>,
    /// User-added root paths.
    pub extra_roots: Vec<String>,
    /// Index every fixed non-system drive.
    pub include_drives: bool,
}

fn settings_view(app: &AppHandle, state: &AppState) -> SettingsView {
    let c = state.config.load();
    let launch_at_startup = autostart::is_autostart(app).unwrap_or(c.general.launch_at_startup);
    SettingsView {
        hotkey_launcher: c.general.hotkey.clone(),
        hotkey_explain: c.general.explain_hotkey.clone(),
        theme: c.general.theme.clone(),
        launch_at_startup,
        find_enabled: c.modes.find.enabled,
        query_enabled: c.modes.query.enabled,
        roots: c.modes.find.roots.clone(),
        extra_roots: c.modes.find.extra_roots.clone(),
        include_drives: c.modes.find.include_drives,
    }
}

fn changed(app: &AppHandle, section: &str) {
    if let Err(err) = app.emit(
        "settings://changed",
        serde_json::json!({ "section": section }),
    ) {
        tracing::warn!(%err, "emit failed");
    }
}

/// Returns the current settings.
#[tauri::command]
pub async fn get_settings(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<SettingsView, CommandError> {
    Ok(settings_view(&app, &state))
}

/// Changes general settings; a hotkey conflict keeps the old hotkey.
#[tauri::command]
pub async fn set_general(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    hotkey_launcher: Option<String>,
    hotkey_explain: Option<String>,
    theme: Option<String>,
    launch_at_startup: Option<bool>,
) -> Result<SettingsView, CommandError> {
    if let Some(t) = &theme
        && !matches!(t.as_str(), "system" | "dark" | "light")
    {
        return Err(CommandError::new(
            "invalid_theme",
            "theme must be system, dark or light",
        ));
    }
    if let Some(h) = &hotkey_explain {
        hotkey::validate(h)?;
    }
    if let Some(new) = &hotkey_launcher {
        hotkey::rebind_launcher(&app, new)?;
    }
    if let Some(on) = launch_at_startup {
        autostart::set_autostart(&app, on)
            .map_err(|e| CommandError::new("autostart_failed", format!("{e:#}")))?;
    }
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|c| {
            if let Some(h) = hotkey_launcher {
                c.general.hotkey = h;
            }
            // ponytail: explain hotkey is stored only; its binding lands with the Explain feature.
            if let Some(h) = hotkey_explain {
                c.general.explain_hotkey = h;
            }
            if let Some(t) = theme {
                c.general.theme = t;
            }
            if let Some(on) = launch_at_startup {
                c.general.launch_at_startup = on;
            }
            Ok(())
        })
    })
    .await?;
    changed(&app, "general");
    Ok(settings_view(&app, &state))
}

/// Replaces the user-added Find roots and/or sets "index all drives", then rescans when Find is on.
#[tauri::command]
pub async fn set_roots(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    roots: Option<Vec<String>>,
    include_drives: Option<bool>,
) -> Result<SettingsView, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        if let Some(bad) = roots
            .iter()
            .flatten()
            .find(|r| !Path::new(r.as_str()).is_dir())
        {
            return Err(CommandError::new(
                "invalid_path",
                format!("not a folder: {bad}"),
            ));
        }
        s.update_config(|c| {
            if let Some(r) = roots {
                c.modes.find.extra_roots = r;
            }
            if let Some(on) = include_drives {
                c.modes.find.include_drives = on;
            }
            Ok(())
        })
    })
    .await?;
    if state.config.load().modes.find.enabled {
        let s = Arc::clone(&state);
        tauri::async_runtime::spawn(async move { state::start_find(&s).await });
    }
    changed(&app, "roots");
    Ok(settings_view(&app, &state))
}

fn scope_of(scope: &str) -> Result<ClearScope, CommandError> {
    match scope {
        "frecency" => Ok(ClearScope::Frecency),
        "nl_cache" => Ok(ClearScope::NlCache),
        "all" => Ok(ClearScope::All),
        _ => Err(CommandError::new(
            "invalid_scope",
            format!("unknown scope {scope}"),
        )),
    }
}

/// Clears stored history. `scope` is `frecency`, `nl_cache` or `all`.
#[tauri::command]
pub async fn clear_history(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    scope: String,
) -> Result<(), CommandError> {
    let sc = scope_of(&scope)?;
    let s = Arc::clone(&state);
    blocking(move || {
        s.store.clear(sc);
        Ok(())
    })
    .await?;
    // The registry cannot downcast to the frecency feature, so tell it by event.
    if sc != ClearScope::NlCache
        && let Err(err) = app.emit("history://cleared", serde_json::json!({ "scope": scope }))
    {
        tracing::warn!(%err, "emit failed");
    }
    Ok(())
}

/// Rebuilds an index. `kind` is `apps` or `files`.
#[tauri::command]
pub async fn reindex(state: State<'_, Arc<AppState>>, kind: String) -> Result<(), CommandError> {
    let s = Arc::clone(&state);
    match kind.as_str() {
        "apps" => {
            tauri::async_runtime::spawn(async move { state::refresh_apps(&s).await });
        }
        "files" => {
            if !state.config.load().modes.find.enabled {
                return Err(CommandError::new("mode_disabled", "Find mode is off"));
            }
            tauri::async_runtime::spawn(async move { state::start_find(&s).await });
        }
        _ => {
            return Err(CommandError::new(
                "invalid_kind",
                format!("unknown kind {kind}"),
            ));
        }
    }
    Ok(())
}

/// About page data.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct About {
    /// App version.
    pub version: String,
    /// License name.
    pub license: &'static str,
    /// Project links.
    pub links: Links,
    /// Log folder.
    pub log_dir: Option<String>,
}

/// Project links.
#[derive(Serialize)]
pub struct Links {
    /// Source repository.
    pub repo: String,
    /// Documentation.
    pub docs: String,
    /// Changelog.
    pub changelog: String,
    /// Releases page.
    pub releases: String,
}

/// Returns version, license, links and log folder.
#[tauri::command]
pub async fn get_about(app: AppHandle) -> Result<About, CommandError> {
    let log_dir = blocking(|| {
        Ok(launcher_core::config::local_data_dir()
            .ok()
            .map(|d| d.join("logs").to_string_lossy().into_owned()))
    })
    .await?;
    let repo = format!("https://github.com/akshit-bansal11/{APP_NAME}");
    Ok(About {
        version: app.package_info().version.to_string(),
        license: "MIT",
        links: Links {
            repo: repo.clone(),
            docs: format!("https://{APP_NAME}.akshitbansal.dev/docs"),
            changelog: format!("{repo}/blob/main/CHANGELOG.md"),
            releases: format!("{repo}/releases"),
        },
        log_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes() {
        assert!(scope_of("all").is_ok());
        assert_eq!(scope_of("x").unwrap_err().code, "invalid_scope");
    }
}
