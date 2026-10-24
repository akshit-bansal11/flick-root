//! General settings, history, reindex, about and window sizing commands.
use super::blocking;
use super::themes::{ActiveTheme, active_theme};
use crate::autostart;
use crate::error::CommandError;
use crate::hotkey;
use crate::state::{self, AppState};
use launcher_core::brand::APP_NAME;
use launcher_core::config::validate_skip;
use launcher_core::store::ClearScope;
use launcher_core::theme::Registry as ThemeRegistry;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};
use win::drives::DriveKind;

/// Settings the UI shows. No secret is ever included.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Launcher hotkey.
    pub hotkey_launcher: String,
    /// Explain hotkey.
    pub hotkey_explain: String,
    /// Launch at login, as the OS reports it.
    pub launch_at_startup: bool,
    /// Daily update check on.
    pub check_updates: bool,
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
    /// Scan fixed drives for programs (exe, msi, apk, ...).
    pub scan_programs: bool,
    /// Excluded drives, folders and name patterns (`[apps] skip`).
    pub program_skip: Vec<String>,
    /// Indexable drives, plus excluded drives that are not connected.
    pub drives: Vec<DriveView>,
    /// Programs currently searchable.
    pub programs_count: usize,
    /// A drive's programs walk stopped at its cap.
    pub programs_capped: bool,
}

/// One drive row of the exclusion settings.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct DriveView {
    /// Drive root, `E:\`.
    pub root: String,
    /// `fixed` or `removable` (`removable` for an unconnected drive, whose kind is unknown).
    pub kind: &'static str,
    /// Connected now.
    pub present: bool,
}

/// Drive rows: every indexable drive, then each drive entry of `skip` that is not connected.
fn drive_views(vols: &[(char, DriveKind)], skip: &[String]) -> Vec<DriveView> {
    let mut out: Vec<DriveView> = vols
        .iter()
        .map(|&(l, k)| DriveView {
            root: format!("{l}:\\"),
            kind: if k == DriveKind::Removable {
                "removable"
            } else {
                "fixed"
            },
            present: true,
        })
        .collect();
    for l in skip.iter().filter_map(|e| drive_letter(e)) {
        if !vols.iter().any(|v| v.0 == l) {
            out.push(DriveView {
                root: format!("{l}:\\"),
                kind: "removable",
                present: false,
            });
        }
    }
    out
}

/// The drive letter (uppercase) when `entry` is exactly a drive such as `E:\`.
fn drive_letter(entry: &str) -> Option<char> {
    let b = entry.as_bytes();
    (b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
        .then(|| b[0].to_ascii_uppercase() as char)
}

/// Rejects a drive entry that is neither a connected indexable drive nor already in `old`.
fn check_drives(new: &[String], old: &[String], present: &[char]) -> Result<(), CommandError> {
    for l in new.iter().filter_map(|e| drive_letter(e)) {
        let known = present.contains(&l) || old.iter().any(|o| drive_letter(o) == Some(l));
        if !known {
            return Err(CommandError::new(
                "invalid_skip",
                "That drive is not connected, so it cannot be excluded.",
            ));
        }
    }
    Ok(())
}

fn settings_view(app: &AppHandle, state: &AppState) -> SettingsView {
    let c = state.config.load();
    let vols: Vec<(char, DriveKind)> = win::drives::indexable_drives()
        .into_iter()
        .map(|d| (d.letter, d.kind))
        .collect();
    let launch_at_startup = autostart::is_autostart(app).unwrap_or(c.general.launch_at_startup);
    SettingsView {
        hotkey_launcher: c.general.hotkey.clone(),
        hotkey_explain: c.general.explain_hotkey.clone(),
        launch_at_startup,
        check_updates: c.general.check_updates,
        find_enabled: c.modes.find.enabled,
        query_enabled: c.modes.query.enabled,
        roots: c.modes.find.roots.clone(),
        extra_roots: c.modes.find.extra_roots.clone(),
        include_drives: c.modes.find.include_drives,
        scan_programs: c.apps.scan_programs,
        program_skip: c.apps.skip.clone(),
        drives: drive_views(&vols, &c.apps.skip),
        programs_count: state.programs.load().paths.len(),
        programs_capped: state.programs_capped.load(Ordering::Relaxed),
    }
}

/// [`settings_view`] on the blocking pool: it asks Windows about the drives.
async fn view(app: &AppHandle, state: &Arc<AppState>) -> Result<SettingsView, CommandError> {
    let (app, state) = (app.clone(), Arc::clone(state));
    blocking(move || Ok(settings_view(&app, &state))).await
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
    view(&app, &state).await
}

/// Scheme, theme slots and the active theme's colours, as the UI applies them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceView {
    /// `system`, `dark` or `light`.
    pub scheme: String,
    /// Theme id used in the dark scheme.
    pub dark_theme: String,
    /// Theme id used in the light scheme.
    pub light_theme: String,
    /// The theme in force right now; `None` only if the registry lost its defaults.
    pub active: Option<ActiveTheme>,
}

impl AppearanceView {
    /// The view of the live config; `active` follows the configured scheme ("system" asks Windows).
    pub fn of(state: &AppState) -> Self {
        let cfg = state.config.load();
        let a = &cfg.appearance;
        Self {
            scheme: a.scheme.clone(),
            dark_theme: a.dark_id().to_owned(),
            light_theme: a.light_id().to_owned(),
            active: active_theme(state),
        }
    }
}

/// Checks the scheme and both slots: known ids, a dark theme in the dark slot, light in the light.
fn check_themes(
    a: &launcher_core::config::Appearance,
    registry: &ThemeRegistry,
) -> Result<(), CommandError> {
    a.validate_themes(&|id| registry.get(id).map(|t| t.resolved.dark))
        .map_err(|m| CommandError::new("invalid_appearance", m))
}

/// Returns the appearance settings.
#[tauri::command]
pub async fn get_appearance(
    state: State<'_, Arc<AppState>>,
) -> Result<AppearanceView, CommandError> {
    Ok(AppearanceView::of(&state))
}

/// Changes any of scheme and the two theme slots after validating the result.
#[tauri::command]
pub async fn set_appearance(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    scheme: Option<String>,
    dark_theme: Option<String>,
    light_theme: Option<String>,
) -> Result<AppearanceView, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|c| {
            let mut next = c.appearance.clone();
            if let Some(sc) = scheme {
                next.scheme = sc;
            }
            if let Some(t) = dark_theme {
                next.dark_theme = Some(t);
            }
            if let Some(t) = light_theme {
                next.light_theme = Some(t);
            }
            check_themes(&next, &s.themes.load())?;
            c.appearance = next;
            Ok(())
        })
    })
    .await?;
    changed(&app, "appearance");
    crate::tray::sync_settings_theme(&app);
    Ok(AppearanceView::of(&state))
}

/// Changes general settings; a hotkey conflict keeps the old hotkey.
#[tauri::command]
pub async fn set_general(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    hotkey_launcher: Option<String>,
    hotkey_explain: Option<String>,
    launch_at_startup: Option<bool>,
    scan_programs: Option<bool>,
    check_updates: Option<bool>,
) -> Result<SettingsView, CommandError> {
    if let Some(h) = &hotkey_explain {
        hotkey::sync_explain(&app, h)?;
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
            if let Some(h) = hotkey_explain {
                c.general.explain_hotkey = h;
            }
            if let Some(on) = launch_at_startup {
                c.general.launch_at_startup = on;
            }
            if let Some(on) = scan_programs {
                c.apps.scan_programs = on;
            }
            if let Some(on) = check_updates {
                c.general.check_updates = on;
            }
            Ok(())
        })
    })
    .await?;
    match scan_programs {
        Some(true) => {
            let s = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                state::run_programs(&s, false, Vec::new(), true).await;
            });
        }
        Some(false) => state::stop_programs(&state),
        None => {}
    }
    changed(&app, "general");
    view(&app, &state).await
}

/// Replaces the exclusion list (`[apps] skip`) after validating it, then applies the change
/// incrementally. Errors use code `invalid_skip`; the message never repeats the entry text.
#[tauri::command]
pub async fn set_program_skip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    skip: Vec<String>,
) -> Result<SettingsView, CommandError> {
    let s = Arc::clone(&state);
    let diff = blocking(move || {
        let new = validate_skip(skip).map_err(|e| {
            CommandError::new("invalid_skip", format!("That exclusion is not valid: {e}."))
        })?;
        let present: Vec<char> = win::drives::indexable_drives()
            .iter()
            .map(|d| d.letter)
            .collect();
        s.update_config(|c| {
            check_drives(&new, &c.apps.skip, &present)?;
            let old = std::mem::replace(&mut c.apps.skip, new.clone());
            Ok(state::skip_diff(&old, &new))
        })
    })
    .await?;
    let s = Arc::clone(&state);
    tauri::async_runtime::spawn(async move { state::apply_skip(&s, diff).await });
    changed(&app, "general");
    view(&app, &state).await
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
    view(&app, &state).await
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
            // Reindex apps also rescans every drive's programs (a no-op while programs are off).
            tauri::async_runtime::spawn(async move {
                state::refresh_apps(&s).await;
                state::run_programs(&s, true, Vec::new(), true).await;
            });
        }
        "files" => {
            if !state.config.load().modes.find.enabled {
                return Err(CommandError::new(
                    "mode_disabled",
                    "File search is off. Turn it on in Settings, Prefixes.",
                ));
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

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn drive_entries_need_a_connected_drive_or_an_existing_entry() {
        let present = ['C', 'E'];
        assert!(check_drives(&strs(&[r"E:\", "node_modules", r"Z:\games"]), &[], &present).is_ok());
        let err = check_drives(&strs(&[r"F:\"]), &[], &present).unwrap_err();
        assert_eq!(err.code, "invalid_skip");
        assert!(!err.message.contains("F:"), "the entry text is not echoed");
        // Already excluded while detached: it may stay.
        assert!(check_drives(&strs(&[r"F:\"]), &strs(&[r"f:\"]), &present).is_ok());
    }

    #[test]
    fn drive_letter_is_a_single_drive_only() {
        assert_eq!(drive_letter(r"e:\"), Some('E'));
        assert_eq!(drive_letter(r"E:\games"), None);
        assert_eq!(drive_letter("E:"), None);
        assert_eq!(drive_letter("node_modules"), None);
    }

    #[test]
    fn drive_rows_list_present_then_missing_excluded() {
        let vols = [('C', DriveKind::Fixed), ('E', DriveKind::Removable)];
        let rows = drive_views(&vols, &strs(&[r"E:\", r"G:\", r"D:\x", "tmp"]));
        let got: Vec<(&str, &str, bool)> = rows
            .iter()
            .map(|r| (r.root.as_str(), r.kind, r.present))
            .collect();
        assert_eq!(
            got,
            [
                (r"C:\", "fixed", true),
                (r"E:\", "removable", true),
                (r"G:\", "removable", false)
            ]
        );
    }

    #[test]
    fn scopes() {
        assert!(scope_of("all").is_ok());
        assert_eq!(scope_of("x").unwrap_err().code, "invalid_scope");
    }

    fn slots(dark: Option<&str>, light: Option<&str>) -> launcher_core::config::Appearance {
        launcher_core::config::Appearance {
            dark_theme: dark.map(str::to_owned),
            light_theme: light.map(str::to_owned),
            ..Default::default()
        }
    }

    #[test]
    fn check_themes_accepts_defaults_and_valid_slots() {
        let reg = ThemeRegistry::builtin().expect("builtin registry");
        assert!(check_themes(&slots(None, None), &reg).is_ok());
        assert!(check_themes(&slots(Some("midnight"), Some("frost")), &reg).is_ok());
    }

    #[test]
    fn check_themes_rejects_wrong_scheme_or_unknown_slot() {
        let reg = ThemeRegistry::builtin().expect("builtin registry");
        for a in [
            slots(Some("frost"), None),
            slots(None, Some("midnight")),
            slots(Some("no-such-theme"), None),
            slots(None, Some("no-such-theme")),
        ] {
            assert_eq!(
                check_themes(&a, &reg).unwrap_err().code,
                "invalid_appearance"
            );
        }
    }

    #[test]
    fn appearance_view_serializes_camel_case_with_active_roles() {
        let reg = ThemeRegistry::builtin().expect("builtin registry");
        // `AppearanceView::of` needs a full AppState; its `active` comes from active_theme_in.
        let view = AppearanceView {
            scheme: "dark".into(),
            dark_theme: "midnight".into(),
            light_theme: "frost".into(),
            active: super::super::themes::active_theme_in(&reg, true, "midnight", "frost"),
        };
        let v = serde_json::to_value(&view).expect("serialize");
        assert_eq!(v["darkTheme"], "midnight");
        assert_eq!(v["lightTheme"], "frost");
        assert_eq!(v["active"]["id"], "midnight");
        assert_eq!(v["active"]["dark"], true);
        assert!(v["active"]["roles"]["accentText"].is_string());
        assert!(v["active"]["roles"]["onAccent"].is_string());
    }
}
