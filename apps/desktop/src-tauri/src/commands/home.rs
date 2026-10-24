//! The `home_items` command: recent app and file picks for the blank launcher.
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
    /// Recent picks, newest first.
    pub items: Vec<Item>,
    /// Pinned item ids (empty until pins exist).
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

/// Recent picks for the blank launcher. Empty while history (the `frecency` feature) is off.
/// The rows join `last_results`, so `launch` can resolve them; an in-flight search keeps its rows.
#[tauri::command]
pub async fn home_items(state: State<'_, Arc<AppState>>) -> Result<HomeView, CommandError> {
    if !state.registry.running_ids().contains(&"frecency") {
        return Ok(HomeView {
            items: Vec::new(),
            pins: Vec::new(),
        });
    }
    let s = Arc::clone(&state);
    let items = super::blocking(move || {
        let rows = s
            .store
            .load_frecency()
            .map_err(|e| CommandError::new("internal", e.to_string()))?;
        let mut items: Vec<Item> = recent_ids(&rows, CANDIDATES)
            .into_iter()
            .filter_map(|(key, is_dir)| resolve(&s, key, is_dir))
            .take(SHOWN)
            .collect();
        for it in &mut items {
            it.payload["home"] = serde_json::json!("recent");
        }
        Ok(items)
    })
    .await?;
    state
        .last_results
        .lock()
        .extend(items.iter().map(|i| (i.id.0.clone(), i.clone())));
    Ok(HomeView {
        items,
        pins: Vec::new(),
    })
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
}
