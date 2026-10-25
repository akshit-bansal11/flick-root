//! The `home_items` and `toggle_pin` commands: pinned and recent app and file picks for the blank
//! launcher.
use crate::error::CommandError;
use crate::state::AppState;
use features::search::{app_row, program_entry, recent_file_row};
use launcher_core::store::FrecencyRow;
use launcher_core::{IconRef, Item};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// Rows shown on the home list.
const SHOWN: usize = 8;
/// Recent keys tried; some no longer resolve (uninstalled app, program list changed).
const CANDIDATES: usize = 32;
const ICON_PX: u32 = 32;

/// Reply to `home_items`.
#[derive(Debug, Serialize)]
pub struct HomeView {
    /// Pinned rows (`payload.home = "pinned"`) in pin order, then recent picks, newest first.
    pub items: Vec<Item>,
    /// Every pinned item id from the config, including ones that no longer resolve to a row.
    pub pins: Vec<String>,
}

/// The newest `limit` app and file rows (key, is-folder), newest first. Only `app:` and `file:` keys
/// are recallable: commands, calc results and answers are not frecency rows and cannot be listed.
fn recent_ids(rows: &[FrecencyRow], limit: usize) -> Vec<(&str, bool)> {
    let mut v: Vec<&FrecencyRow> = rows
        .iter()
        .filter(|r| r.item_key.starts_with("app:") || r.item_key.starts_with("file:"))
        .collect();
    v.sort_by_key(|r| std::cmp::Reverse(r.last_used_ms));
    v.into_iter()
        .take(limit)
        .map(|r| (r.item_key.as_str(), r.kind == "folder"))
        .collect()
}

/// Resolves one key to a row, or `None` when the app is gone. Files are not checked on disk.
fn resolve(state: &AppState, key: &str, is_dir: bool) -> Option<Item> {
    if let Some(parsing) = key.strip_prefix("app:") {
        let am = state.app_match.load();
        if let Some(i) = am.entries.iter().position(|a| a.parsing_name == parsing) {
            let entry = am.entries.get(i)?;
            let mut it = app_row(entry, i, false, false);
            it.icon =
                crate::icons::icon_data_url(&state.icons, entry, ICON_PX).map(IconRef::DataUrl);
            return Some(it);
        }
        let set = state.programs.load();
        let i = set.paths.iter().position(|p| &**p == parsing)?;
        let entry = program_entry(parsing);
        let removable = set.removable.get(i).copied().unwrap_or(false);
        let mut it = app_row(&entry, i, true, removable);
        it.icon = crate::icons::icon_data_url(&state.icons, &entry, ICON_PX).map(IconRef::DataUrl);
        return Some(it);
    }
    key.strip_prefix("file:")
        .map(|p| recent_file_row(p, is_dir))
}

/// Keys for the home list, pinned first (in pin order, `payload.home = "pinned"`), then the recents
/// that are not pinned (`"recent"`). A pinned key counts as a folder when the history says so; a
/// pinned `file:` key absent from the history (history off, or too old) asks `is_dir` with its path.
fn home_keys<'a>(
    pins: &'a [String],
    recents: &[(&'a str, bool)],
    is_dir: impl Fn(&str) -> bool,
) -> Vec<(&'a str, bool, &'static str)> {
    let pinned = pins.iter().map(|p| {
        let dir = match recents.iter().find(|&&(k, _)| k == p.as_str()) {
            Some(&(_, d)) => d,
            None => p.strip_prefix("file:").is_some_and(&is_dir),
        };
        (p.as_str(), dir, "pinned")
    });
    let rest = recents
        .iter()
        .filter(|&&(k, _)| !pins.iter().any(|p| p.as_str() == k))
        .map(|&(k, d)| (k, d, "recent"));
    pinned.chain(rest).collect()
}

/// Builds the home list: pinned rows first (even while history is off), then up to [`SHOWN`]
/// recents. A pinned id that no longer resolves is skipped here and kept in the config.
/// The rows join `last_results`, so `launch` can resolve them; an in-flight search keeps its rows.
async fn view(state: &Arc<AppState>) -> Result<HomeView, CommandError> {
    let s = Arc::clone(state);
    let (items, pins) = super::blocking(move || {
        let pins = s.config.load().home.pins.clone();
        let rows = if s.registry.running_ids().contains(&"frecency") {
            s.store
                .load_frecency()
                .map_err(|e| CommandError::new("internal", e.to_string()))?
        } else {
            Vec::new()
        };
        let recents = recent_ids(&rows, CANDIDATES);
        let mut items: Vec<Item> = Vec::new();
        let mut recent_shown = 0;
        let on_disk_dir = |p: &str| std::path::Path::new(p).is_dir();
        for (key, is_dir, tag) in home_keys(&pins, &recents, on_disk_dir) {
            if tag == "recent" && recent_shown >= SHOWN {
                break;
            }
            // A pinned row just pinned from a search is cached with its real kind and icon.
            let cached = if tag == "pinned" {
                s.last_results.lock().get(key).cloned()
            } else {
                None
            };
            let Some(mut it) = cached.or_else(|| resolve(&s, key, is_dir)) else {
                continue;
            };
            mark_home(&mut it, tag);
            recent_shown += usize::from(tag == "recent");
            items.push(it);
        }
        Ok((items, pins))
    })
    .await?;
    state
        .last_results
        .lock()
        .extend(items.iter().map(|i| (i.id.0.clone(), i.clone())));
    Ok(HomeView { items, pins })
}

/// Pinned and recent picks for the blank launcher.
#[tauri::command]
pub async fn home_items(state: State<'_, Arc<AppState>>) -> Result<HomeView, CommandError> {
    view(&state).await
}

/// Pins `item_id` (an `app:` or `file:` row of the current results), or unpins it when pinned, then
/// returns the fresh home list. At most 24 pins; refusals carry a user-readable message.
#[tauri::command]
pub async fn toggle_pin(
    state: State<'_, Arc<AppState>>,
    item_id: String,
) -> Result<HomeView, CommandError> {
    let known = state.last_results.lock().contains_key(&item_id);
    let s = Arc::clone(&state);
    super::blocking(move || {
        s.update_config(|c| {
            c.home
                .toggle(&item_id, known)
                .map(|_| ())
                .map_err(|e| CommandError::new("pin_failed", e.to_string()))
        })
    })
    .await?;
    view(&state).await
}

/// Marks a row as a home row (`payload.home = tag`). A null payload becomes an object; any
/// other non-object payload is left as it is, never panicking on it.
fn mark_home(it: &mut Item, tag: &str) {
    if it.payload.is_null() {
        it.payload = serde_json::json!({});
    }
    if let Some(obj) = it.payload.as_object_mut() {
        obj.insert("home".into(), serde_json::json!(tag));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str, kind: &str, last: i64) -> FrecencyRow {
        FrecencyRow {
            item_key: key.into(),
            kind: kind.into(),
            score: 1.0,
            uses: 1,
            last_used_ms: last,
        }
    }

    fn keys(rows: &[FrecencyRow], limit: usize) -> Vec<&str> {
        recent_ids(rows, limit)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    #[test]
    fn recent_ids_sorts_newest_first() {
        let rows = [
            row("app:a", "app", 10),
            row("file:b", "file", 30),
            row("app:c", "app", 20),
        ];
        assert_eq!(keys(&rows, 8), ["file:b", "app:c", "app:a"]);
    }

    #[test]
    fn recent_ids_keeps_only_app_and_file_keys() {
        let rows = [
            row("cmd:lock", "command", 50),
            row("calc:1+1", "result", 40),
            row("app:a", "app", 10),
            row("file:b", "file", 5),
        ];
        assert_eq!(keys(&rows, 8), ["app:a", "file:b"]);
    }

    #[test]
    fn recent_ids_caps_at_limit() {
        let rows = [
            row("app:a", "app", 1),
            row("app:b", "app", 2),
            row("app:c", "app", 3),
        ];
        assert_eq!(keys(&rows, 2), ["app:c", "app:b"]);
        assert!(recent_ids(&rows, 0).is_empty());
    }

    #[test]
    fn recent_ids_folder_kind_sets_is_dir() {
        let rows = [
            row("file:C:/d", "folder", 2),
            row("file:C:/f.txt", "file", 1),
        ];
        assert_eq!(
            recent_ids(&rows, 8),
            [("file:C:/d", true), ("file:C:/f.txt", false)]
        );
    }

    fn item_with(payload: serde_json::Value) -> Item {
        use launcher_core::{ItemId, ItemKind};
        Item {
            id: ItemId("app:x".into()),
            kind: ItemKind::App,
            title: "x".into(),
            subtitle: String::new(),
            icon: None,
            score: 1.0,
            source: "apps".into(),
            actions: Vec::new(),
            payload,
        }
    }

    #[test]
    fn mark_home_turns_null_into_an_object() {
        let mut it = item_with(serde_json::Value::Null);
        mark_home(&mut it, "recent");
        assert_eq!(it.payload, serde_json::json!({ "home": "recent" }));
    }

    #[test]
    fn mark_home_inserts_into_an_object_and_keeps_its_keys() {
        let mut it = item_with(serde_json::json!({ "path": "C:/a" }));
        mark_home(&mut it, "pinned");
        assert_eq!(
            it.payload,
            serde_json::json!({ "path": "C:/a", "home": "pinned" })
        );
    }

    #[test]
    fn mark_home_leaves_other_payloads_unchanged() {
        for p in [
            serde_json::json!("s"),
            serde_json::json!([1, 2]),
            serde_json::json!(7),
        ] {
            let mut it = item_with(p.clone());
            mark_home(&mut it, "recent");
            assert_eq!(it.payload, p);
        }
    }

    #[test]
    fn home_keys_puts_pins_first_and_dedupes_recents() {
        let pins = ["file:C:/d".to_owned(), "app:z".to_owned()];
        let recents = [("app:a", false), ("file:C:/d", true), ("app:z", false)];
        assert_eq!(
            home_keys(&pins, &recents, |_| false),
            [
                ("file:C:/d", true, "pinned"),
                ("app:z", false, "pinned"),
                ("app:a", false, "recent"),
            ]
        );
    }

    #[test]
    fn home_keys_probes_pinned_files_missing_from_history() {
        let pins = [
            "file:C:/gone".to_owned(),
            "file:C:/known.txt".to_owned(),
            "app:x".to_owned(),
        ];
        // History says known.txt is a file; the probe would say dir, so it must not be asked.
        let recents = [("file:C:/known.txt", false)];
        let probe = |p: &str| p == "C:/gone" || p == "C:/known.txt" || p == "x";
        assert_eq!(
            home_keys(&pins, &recents, probe),
            [
                ("file:C:/gone", true, "pinned"),
                ("file:C:/known.txt", false, "pinned"),
                ("app:x", false, "pinned"),
            ]
        );
    }

    #[test]
    fn home_keys_without_pins_is_the_recents_and_without_recents_the_pins() {
        let recents = [("app:a", false)];
        assert_eq!(
            home_keys(&[], &recents, |_| false),
            [("app:a", false, "recent")]
        );
        let pins = ["app:p".to_owned()];
        assert_eq!(
            home_keys(&pins, &[], |_| false),
            [("app:p", false, "pinned")]
        );
    }
}
