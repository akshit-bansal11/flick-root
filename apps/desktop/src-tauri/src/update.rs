//! Update-available notice: one GitHub release lookup, no timers.
//!
//! Runs once shortly after startup and again on launcher open when the last check is over a day
//! old. The tray item forces a check (or opens the release page when one is already known).
use crate::state::{AppState, Busy};
use launcher_core::brand::{APP_DISPLAY_NAME, APP_NAME, REPO};
use launcher_core::config::local_data_dir;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

const STALE_SECS: i64 = 24 * 60 * 60;
const STARTUP_DELAY: Duration = Duration::from_secs(20);
const TIMEOUT: Duration = Duration::from_secs(10);

/// Persisted in `update.json`.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq)]
#[serde(default)]
struct Stored {
    last_check_unix: i64,
    notified_version: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

#[derive(Clone)]
struct Found {
    version: String,
    url: String,
}

/// Managed state: the single-flight guard, the tray item and the update found so far.
pub struct UpdateState {
    busy: Busy,
    item: MenuItem<Wry>,
    known: Mutex<Option<Found>>,
}

/// Registers the update state; `item` is the tray entry whose text changes when an update is found.
pub fn init(app: &tauri::App, item: MenuItem<Wry>) {
    app.manage(UpdateState {
        busy: Busy::default(),
        item,
        known: Mutex::new(None),
    });
}

/// Startup check: after a short delay, off the setup path.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        run(&app, false).await;
    });
}

/// Called on launcher open; returns at once, and does nothing when off or checked within a day.
pub fn check_if_stale(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run(&app, false).await });
}

/// Tray click: opens the known release page, otherwise checks now.
pub fn on_tray(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(st) = app.try_state::<UpdateState>() else {
            return;
        };
        let known = st.known.lock().clone();
        let Some(f) = known else {
            run(&app, true).await;
            return;
        };
        // Single-flight: a second click while downloading does nothing.
        let Some(_guard) = st.busy.try_begin() else {
            return;
        };
        toast(
            &app,
            format!("Downloading {APP_DISPLAY_NAME} {}…", f.version),
            "It restarts by itself when the update is installed.",
        );
        if let Err(err) = install(&app).await {
            tracing::warn!(?err, "update install failed");
            toast(
                &app,
                APP_DISPLAY_NAME.to_string(),
                "Couldn't install the update. Opening the download page.",
            );
            if let Err(err) = crate::commands::launch::open_link(f.url).await {
                tracing::warn!(?err, "opening release page failed");
            }
        }
    });
}

/// Downloads, verifies and installs the signed update. On Windows the app exits (after the same
/// shutdown as a normal quit) and the passive installer relaunches it, so success never returns.
async fn install(app: &AppHandle) -> anyhow::Result<()> {
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .on_before_exit(move || crate::state::shutdown(&handle))
        .build()?;
    let Some(update) = updater.check().await? else {
        anyhow::bail!("no signed update published yet");
    };
    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(())
}

fn path() -> Option<PathBuf> {
    local_data_dir().ok().map(|d| d.join("update.json"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// True when the last check is over 24 h old (or the clock went backwards).
fn is_stale(now: i64, last: i64) -> bool {
    last > now || now - last >= STALE_SECS
}

/// `MAJOR.MINOR.PATCH` of a plain release tag; `None` for garbage or a pre-release.
fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let t = tag.trim().trim_start_matches('v');
    if t.contains('-') {
        return None;
    }
    let core = t.split('+').next()?;
    let mut it = core.split('.');
    let v = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    );
    it.next().is_none().then_some(v)
}

fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn releases_prefix() -> String {
    format!("https://github.com/{REPO}/releases/")
}

/// The release page from GitHub when it is on our repo, else the latest-release page.
fn safe_url(html_url: &str) -> String {
    let prefix = releases_prefix();
    if html_url.starts_with(&prefix) {
        html_url.to_string()
    } else {
        format!("{prefix}latest")
    }
}

async fn fetch_latest() -> anyhow::Result<Option<Found>> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(format!("{APP_NAME}/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let resp = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let rel: Release = resp.error_for_status()?.json().await?;
    if !is_newer(&rel.tag_name, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    Ok(Some(Found {
        version: rel.tag_name.trim_start_matches('v').to_string(),
        url: safe_url(&rel.html_url),
    }))
}

fn toast(app: &AppHandle, title: String, body: &str) {
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(%err, "update notification failed");
    }
}

async fn load(p: Option<PathBuf>) -> Stored {
    tauri::async_runtime::spawn_blocking(move || {
        p.and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

async fn save(p: Option<PathBuf>, stored: Stored) {
    let Some(p) = p else { return };
    let res = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        std::fs::write(p, serde_json::to_vec(&stored)?)?;
        Ok(())
    })
    .await;
    match res {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::warn!(%err, "saving update state failed"),
        Err(err) => tracing::warn!(%err, "update state task failed"),
    }
}

/// One check. `manual` ignores the setting and the 24 h gate and reports every outcome.
async fn run(app: &AppHandle, manual: bool) {
    let Some(st) = app.try_state::<UpdateState>() else {
        return;
    };
    if !manual {
        let on = app
            .try_state::<Arc<AppState>>()
            .is_some_and(|s| s.config.load().general.check_updates);
        if !on {
            return;
        }
    }
    let Some(_guard) = st.busy.try_begin() else {
        return;
    };
    let p = path();
    let mut stored = load(p.clone()).await;
    let now = now_unix();
    if !manual && !is_stale(now, stored.last_check_unix) {
        return;
    }
    let result = fetch_latest().await;
    stored.last_check_unix = now;
    match result {
        Ok(Some(f)) => {
            let title = format!("{APP_DISPLAY_NAME} {} is available", f.version);
            if let Err(err) = st
                .item
                .set_text(format!("Install {APP_DISPLAY_NAME} {}", f.version))
            {
                tracing::warn!(%err, "tray text update failed");
            }
            if manual || stored.notified_version.as_deref() != Some(f.version.as_str()) {
                toast(
                    app,
                    title,
                    "Open the tray menu and choose Install to update.",
                );
                stored.notified_version = Some(f.version.clone());
            }
            *st.known.lock() = Some(f);
        }
        Ok(None) => {
            if manual {
                toast(
                    app,
                    APP_DISPLAY_NAME.to_string(),
                    "You're on the latest version",
                );
            }
        }
        Err(err) => {
            tracing::warn!(%err, "update check failed");
            if manual {
                toast(
                    app,
                    APP_DISPLAY_NAME.to_string(),
                    "Couldn't check for updates",
                );
            }
        }
    }
    save(p, stored).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v1.2.4", "1.2.3"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.2", "1.2.3"));
        assert!(!is_newer("v1.3.0-beta.1", "1.2.3"));
        assert!(is_newer("1.3.0+build5", "1.2.3"));
        for bad in ["", "v", "latest", "1.2", "1.2.3.4", "a.b.c", "1.2.x"] {
            assert!(!is_newer(bad, "0.0.1"), "{bad}");
        }
    }

    #[test]
    fn url_validation() {
        let ok = "https://github.com/akshit-bansal11/flick/releases/tag/v1.0.0";
        assert_eq!(safe_url(ok), ok);
        let fallback = "https://github.com/akshit-bansal11/flick/releases/latest";
        assert_eq!(safe_url("https://evil.example/releases/"), fallback);
        assert_eq!(
            safe_url("https://github.com/akshit-bansal11/flick-evil/releases/x"),
            fallback
        );
        assert_eq!(safe_url(""), fallback);
    }

    #[test]
    fn staleness() {
        let day = STALE_SECS;
        assert!(is_stale(day, 0));
        assert!(is_stale(day + 5, 0));
        assert!(!is_stale(day - 1, 0));
        assert!(!is_stale(100, 100));
        assert!(is_stale(100, 200));
    }
}
