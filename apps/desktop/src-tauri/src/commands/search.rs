//! The `search` command.
use crate::error::CommandError;
use crate::state::AppState;
use features::search::{SearchScope, command_search_scoped};
use launcher_core::{IconRef, Input, Item, Mode};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
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
    scope: Option<SearchScope>,
) -> Result<SearchResponse, CommandError> {
    let started = Instant::now();
    state.latest_seq.fetch_max(seq, Ordering::SeqCst);
    let input = Input {
        raw: text.clone(),
        text,
        mode,
        seq,
    };
    let items = match mode {
        Mode::Query => Vec::new(),
        Mode::Command => command(&state, &input, scope.unwrap_or_default()),
        Mode::Find => {
            if !state.config.load().modes.find.enabled {
                return Err(CommandError::new("mode_disabled", "Find mode is off"));
            }
            let files = state.files.clone();
            let overlay = state.overlay.load_full();
            let inp = input.clone();
            let found = tauri::async_runtime::spawn_blocking(move || {
                features::find_search(&files, &overlay, &inp, K)
            })
            .await
            .map_err(|e| CommandError::new("internal", e.to_string()))?;
            // Feature items (the natural-language finder row) sit above the file hits.
            let mut items = state.registry.query_all(&input);
            items.extend(found);
            items
        }
    };
    let micros = started.elapsed().as_micros() as u64;
    let budget_us = if mode == Mode::Find { 25_000 } else { 2_000 };
    if micros > budget_us {
        tracing::info!(?mode, items = items.len(), micros, "slow search");
    } else {
        tracing::debug!(?mode, items = items.len(), micros, "search");
    }
    if state.latest_seq.load(Ordering::SeqCst) > seq {
        return Ok(reply(seq, Vec::new()));
    }
    *state.last_results.lock() = items.iter().map(|i| (i.id.0.clone(), i.clone())).collect();
    *state.last_input.lock() = Some(input);
    Ok(reply(seq, items))
}

fn command(state: &Arc<AppState>, input: &Input, scope: SearchScope) -> Vec<Item> {
    let am = state.app_match.load();
    let mut items = command_search_scoped(
        &am.entries,
        am.programs_from,
        scope,
        &am.index,
        &mut state.matcher.lock(),
        &state.registry,
        input,
        K,
    );
    let mut missing = Vec::new();
    for it in items.iter_mut().filter(|i| i.source == "apps") {
        let entry = it
            .payload
            .get("appId")
            .and_then(|v| v.as_str())
            .and_then(|id| am.entries.iter().find(|a| a.id == id));
        let Some(entry) = entry else { continue };
        if let Some(url) = crate::icons::icon_data_url(&state.icons, entry, ICON_PX) {
            it.icon = Some(IconRef::DataUrl(url));
        } else if it.payload.get("program").is_some() {
            missing.push(entry.clone());
        }
    }
    if !missing.is_empty() {
        // Only the K shown program rows are extracted, off the keystroke path; shown on the next search.
        let st = state.clone();
        tauri::async_runtime::spawn_blocking(move || st.icons.extract_and_memo(&missing, ICON_PX));
    }
    items
}
