//! The `search` command.
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::{IconRef, Input, Item, Mode};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::State;

/// Results returned at most; icons are only read for these.
const K: usize = 8;
const ICON_PX: u32 = 32;

/// Reply to `search`.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Echo of the request sequence number.
    pub seq: u64,
    /// Ranked items.
    pub items: Vec<Item>,
    /// Group label, if any.
    pub group: Option<String>,
}

fn reply(seq: u64, items: Vec<Item>) -> SearchResponse {
    SearchResponse {
        seq,
        items,
        group: None,
    }
}

/// Searches in the given mode. Superseded calls return no items.
#[tauri::command]
pub async fn search(
    state: State<'_, Arc<AppState>>,
    seq: u64,
    mode: Mode,
    text: String,
) -> Result<SearchResponse, CommandError> {
    state.latest_seq.fetch_max(seq, Ordering::SeqCst);
    let input = Input {
        raw: text.clone(),
        text,
        mode,
        seq,
    };
    let items = match mode {
        Mode::Query => Vec::new(),
        Mode::Command => command(&state, &input),
        Mode::Find => {
            if !state.config.load().modes.find.enabled {
                return Err(CommandError::new("mode_disabled", "Find mode is off"));
            }
            let files = state.files.clone();
            let overlay = state.overlay.load_full();
            let inp = input.clone();
            tauri::async_runtime::spawn_blocking(move || {
                features::find_search(&files, &overlay, &inp, K)
            })
            .await
            .map_err(|e| CommandError::new("internal", e.to_string()))?
        }
    };
    if state.latest_seq.load(Ordering::SeqCst) > seq {
        return Ok(reply(seq, Vec::new()));
    }
    *state.last_results.lock() = items.iter().map(|i| (i.id.0.clone(), i.clone())).collect();
    *state.last_input.lock() = Some(input);
    Ok(reply(seq, items))
}

fn command(state: &AppState, input: &Input) -> Vec<Item> {
    let am = state.app_match.load();
    let mut items = features::command_search(
        &am.entries,
        &am.index,
        &mut state.matcher.lock(),
        &state.registry,
        input,
        K,
    );
    for it in items.iter_mut().filter(|i| i.source == "apps") {
        let entry = it
            .payload
            .get("appId")
            .and_then(|v| v.as_str())
            .and_then(|id| am.entries.iter().find(|a| a.id == id));
        if let Some(url) = entry.and_then(|e| crate::icons::icon_data_url(&state.icons, e, ICON_PX))
        {
            it.icon = Some(IconRef::DataUrl(url));
        }
    }
    items
}
