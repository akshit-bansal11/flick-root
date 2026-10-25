//! The `search` command.
use crate::error::CommandError;
use crate::state::AppState;
use features::registry::FeatureSel;
use features::search::{
    Sources, command_search_scoped, compose_open, find_search, merge_items, program_entry,
    program_items,
};
use launcher_core::{IconRef, Input, Item, Mode};
use serde::Serialize;
use std::collections::HashMap;
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

/// Searches the given sources (wire ids, see `Sources::parse`). `mode` filters which features answer.
/// Superseded calls return no items.
#[tauri::command]
pub async fn search(
    state: State<'_, Arc<AppState>>,
    seq: u64,
    mode: Mode,
    text: String,
    sources: Vec<String>,
) -> Result<SearchResponse, CommandError> {
    let started = Instant::now();
    state.latest_seq.fetch_max(seq, Ordering::SeqCst);
    let src = Sources::parse(&sources, |id| state.registry.knows(id))
        .map_err(|id| CommandError::new("invalid_source", format!("unknown source {id}")))?;
    let input = Input {
        raw: text.clone(),
        text,
        mode,
        seq,
    };
    let items = if mode == Mode::Query {
        Vec::new()
    } else {
        // A disabled file index or empty text contributes no files and is not an error.
        let files_task = (src.files
            && state.config.load().modes.find.enabled
            && !input.text.is_empty())
        .then(|| {
            let files = state.files.clone();
            let overlay = state.overlay.load_full();
            let inp = input.clone();
            tauri::async_runtime::spawn_blocking(move || find_search(&files, &overlay, &inp, K))
        });
        let cmd = Sources {
            apps: src.apps,
            files: false,
            features: src.features.clone(),
        };
        let items = if cmd.apps || cmd.features != FeatureSel::None {
            command(&state, &input, &cmd).await?
        } else {
            Vec::new()
        };
        if src.files {
            let files = match files_task {
                Some(t) => t
                    .await
                    .map_err(|e| CommandError::new("internal", e.to_string()))?,
                None => Vec::new(),
            };
            compose_open(items, files, K)
        } else {
            items
        }
    };
    let micros = started.elapsed().as_micros() as u64;
    let budget_us = if src.files { 25_000 } else { 2_000 };
    if micros > budget_us {
        tracing::info!(?mode, items = items.len(), micros, "slow search");
    } else {
        tracing::debug!(?mode, items = items.len(), micros, "search");
    }
    if state.latest_seq.load(Ordering::SeqCst) > seq {
        return Ok(reply(seq, Vec::new()));
    }
    {
        let mut last = state.last_results.lock();
        *last = replace_keeping_home(&last, &items);
    }
    *state.last_input.lock() = Some(input);
    Ok(reply(seq, items))
}

/// The new `last_results` for a search: its `items`, plus the home rows (marked by `home_items`,
/// at most a handful) already in `last`, so the blank launcher's cached list stays actionable
/// after Esc or Backspace. A new item wins over a home row with the same id.
fn replace_keeping_home(last: &HashMap<String, Item>, items: &[Item]) -> HashMap<String, Item> {
    let mut next: HashMap<String, Item> =
        items.iter().map(|i| (i.id.0.clone(), i.clone())).collect();
    for (k, i) in last.iter().filter(|(_, i)| i.payload.get("home").is_some()) {
        next.entry(k.clone()).or_insert_with(|| i.clone());
    }
    next
}

/// Apps and features run inline (2 ms budget); the programs index is matched on the blocking pool.
async fn command(
    state: &Arc<AppState>,
    input: &Input,
    sources: &Sources,
) -> Result<Vec<Item>, CommandError> {
    let am = state.app_match.load();
    let items = command_search_scoped(
        &am.entries,
        sources,
        &am.index,
        &mut state.matcher.lock(),
        &state.registry,
        input,
        K,
    );
    let programs = if !sources.apps || input.text.is_empty() {
        Vec::new()
    } else {
        let st = state.clone();
        let text = input.text.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let set = st.programs.load();
            program_items(
                &set.paths,
                &set.removable,
                &set.index,
                &mut st.prog_matcher.lock(),
                &text,
                K,
            )
        })
        .await
        .map_err(|e| CommandError::new("internal", e.to_string()))?
    };
    let mut items = merge_items(items, programs, &input.text, K);
    let set = state.programs.load();
    let mut missing = Vec::new();
    for it in items.iter_mut().filter(|i| i.source == "apps") {
        let program = it.payload.get("program").is_some();
        let idx = it
            .payload
            .get("entry")
            .and_then(|v| v.as_u64())
            .map(|i| i as usize);
        let Some(idx) = idx else { continue };
        let entry = if program {
            set.paths.get(idx).map(|p| program_entry(p))
        } else {
            am.entries.get(idx).cloned()
        };
        let Some(entry) = entry else { continue };
        if let Some(url) = crate::icons::icon_data_url(&state.icons, &entry, ICON_PX) {
            it.icon = Some(IconRef::DataUrl(url));
        } else if program {
            missing.push(entry);
        }
    }
    let missing = state.icons.claim(missing);
    if !missing.is_empty() {
        // Only the K shown program rows are extracted, off the keystroke path; shown on the next search.
        let st = state.clone();
        tauri::async_runtime::spawn_blocking(move || st.icons.extract_and_memo(&missing, ICON_PX));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{ItemId, ItemKind};

    fn item(id: &str, payload: serde_json::Value) -> Item {
        Item {
            id: ItemId(id.into()),
            kind: ItemKind::App,
            title: id.into(),
            subtitle: String::new(),
            icon: None,
            score: 1.0,
            source: "apps".into(),
            actions: Vec::new(),
            payload,
        }
    }

    fn map(items: Vec<Item>) -> HashMap<String, Item> {
        items.into_iter().map(|i| (i.id.0.clone(), i)).collect()
    }

    fn home() -> serde_json::Value {
        serde_json::json!({ "home": "recent" })
    }

    fn sorted_keys(m: &HashMap<String, Item>) -> Vec<&str> {
        let mut k: Vec<&str> = m.keys().map(String::as_str).collect();
        k.sort_unstable();
        k
    }

    #[test]
    fn keeps_home_rows_and_drops_other_old_rows() {
        let last = map(vec![
            item("h", home()),
            item("old", serde_json::Value::Null),
        ]);
        let next = replace_keeping_home(&last, &[item("n", serde_json::Value::Null)]);
        assert_eq!(sorted_keys(&next), ["h", "n"]);
    }

    #[test]
    fn a_new_item_wins_an_id_clash() {
        let last = map(vec![item("a", home())]);
        let fresh = item("a", serde_json::json!({ "fresh": true }));
        let next = replace_keeping_home(&last, &[fresh]);
        assert_eq!(next["a"].payload, serde_json::json!({ "fresh": true }));
    }

    #[test]
    fn empty_last_yields_just_the_new_items() {
        let next = replace_keeping_home(&HashMap::new(), &[item("n", serde_json::Value::Null)]);
        assert_eq!(sorted_keys(&next), ["n"]);
    }
}
