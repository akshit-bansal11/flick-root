//! Search composition for Command and Find modes.

use crate::registry::Registry;
use index_apps::AppEntry;
use index_files::{FileHit, FileIndex, FindQuery, Overlay};
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use launcher_core::{ActionKind, Input, Item, ItemId, ItemKind};
use std::cmp::Ordering;

/// Which sources a Command-mode search draws from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    /// Apps, programs and feature items.
    #[default]
    All,
    /// Apps and programs only.
    Apps,
    /// Feature items only.
    Commands,
}

/// Program rows rank below real apps by this factor.
const PROGRAM_FACTOR: f32 = 0.75;
/// Program extensions that can run elevated.
const ADMIN_EXTS: &[&str] = &["exe", "msi"];

fn app_item(a: &AppEntry, score: f32, program: bool) -> Item {
    let mut actions = vec![ActionKind::Primary, ActionKind::Location];
    let mut payload = serde_json::json!({ "appId": a.id, "parsingName": a.parsing_name });
    let mut subtitle = String::new();
    let mut admin = true;
    if program {
        let path = std::path::Path::new(&a.parsing_name);
        subtitle = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        admin = ADMIN_EXTS.contains(&ext.as_str());
        payload["program"] = serde_json::json!(true);
    }
    if admin {
        actions.insert(1, ActionKind::Admin);
    }
    Item {
        id: ItemId(format!("app:{}", a.parsing_name)),
        kind: ItemKind::App,
        title: a.name.clone(),
        subtitle,
        icon: None,
        score,
        source: "apps".into(),
        actions,
        payload,
    }
}

/// Same tie-break as `launcher_core::ranking::rank`: shorter title, then alphabetical.
fn by_score(a: &Item, b: &Item) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| a.title.chars().count().cmp(&b.title.chars().count()))
        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
}

/// Command-mode results over all sources; see [`command_search_scoped`].
pub fn command_search(
    apps: &[AppEntry],
    app_index: &MatchIndex,
    matcher: &mut FuzzyMatcher,
    registry: &Registry,
    input: &Input,
    k: usize,
) -> Vec<Item> {
    command_search_scoped(
        apps,
        apps.len(),
        SearchScope::All,
        app_index,
        matcher,
        registry,
        input,
        k,
    )
}

/// Command-mode results: apps (fuzzy, or most-used when `input.text` is empty) plus feature items, ranked with feature boosts, at most `k`.
/// `apps[programs_from..]` are scanned programs: they rank below real apps and are never listed for empty input.
#[allow(clippy::too_many_arguments)]
pub fn command_search_scoped(
    apps: &[AppEntry],
    programs_from: usize,
    scope: SearchScope,
    app_index: &MatchIndex,
    matcher: &mut FuzzyMatcher,
    registry: &Registry,
    input: &Input,
    k: usize,
) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    if scope != SearchScope::Commands {
        if input.text.is_empty() {
            items.extend(
                apps.iter()
                    .take(programs_from)
                    .map(|a| app_item(a, 0.0, false)),
            );
        } else {
            // Extra hits so programs cannot crowd real apps out of the top k.
            let hits = matcher.top_k(&input.text, app_index, k.saturating_mul(4).max(k));
            let max = hits.iter().map(|h| h.score as f32).fold(0.0_f32, f32::max);
            // Nucleo scores are unbounded; feature items score 0..1. Squash apps into 0..=APP_CEIL so an
            // exact feature keyword match (1.0) outranks a fuzzy app hit, while apps still order among themselves.
            // ponytail: relative-to-top squash, a fixed calibration if apps ever need absolute confidence.
            const APP_CEIL: f32 = 0.9;
            items.extend(hits.into_iter().filter_map(|h| {
                let a = apps.get(h.idx)?;
                let program = h.idx >= programs_from;
                let f = if program { PROGRAM_FACTOR } else { 1.0 };
                let score = if max > 0.0 {
                    APP_CEIL * f * h.score as f32 / max
                } else {
                    0.0
                };
                Some(app_item(a, score, program))
            }));
        }
    }
    if scope != SearchScope::Apps {
        items.extend(registry.query_all(input));
        for it in &mut items {
            it.score += registry.rank_boost_all(input, it);
        }
    }
    if k > 0 && items.len() > k {
        items.select_nth_unstable_by(k - 1, by_score);
    }
    items.truncate(k);
    items.sort_by(by_score);
    items
}

pub(crate) fn file_item(h: FileHit) -> Item {
    let path = h.path.to_string_lossy().into_owned();
    Item {
        id: ItemId(format!("file:{path}")),
        kind: if h.is_dir {
            ItemKind::Folder
        } else {
            ItemKind::File
        },
        title: h.name,
        subtitle: h.parent,
        icon: None,
        score: h.score as f32,
        source: "files".into(),
        actions: vec![
            ActionKind::Primary,
            ActionKind::Location,
            ActionKind::CopyPath,
        ],
        payload: serde_json::json!({ "path": path }),
    }
}

/// Find-mode results from the file index, at most `k`.
pub fn find_search(index: &FileIndex, overlay: &Overlay, input: &Input, k: usize) -> Vec<Item> {
    let q = FindQuery {
        text: input.text.clone(),
        limit: k,
        ..FindQuery::default()
    };
    index.find(overlay, &q).into_iter().map(file_item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::Mode;

    fn app(name: &str) -> AppEntry {
        AppEntry {
            id: name.into(),
            name: name.into(),
            parsing_name: format!("p.{name}"),
            target: None,
            is_packaged: false,
        }
    }

    fn input(text: &str) -> Input {
        Input {
            raw: text.into(),
            text: text.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    #[test]
    fn fuzzy_apps_and_empty_query() {
        let apps = vec![app("Terminal"), app("Code"), app("Notepad")];
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        let idx = MatchIndex::new(&names);
        let mut m = FuzzyMatcher::new();
        let r = Registry::new();
        let got = command_search(&apps, &idx, &mut m, &r, &input("code"), 5);
        assert_eq!(got[0].title, "Code");
        assert_eq!(got[0].id.0, "app:p.Code");
        let all = command_search(&apps, &idx, &mut m, &r, &input(""), 2);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn programs_rank_below_apps_and_scope_filters() {
        let apps = vec![app("Code"), app("Code Helper")];
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        let idx = MatchIndex::new(&names);
        let mut m = FuzzyMatcher::new();
        let r = Registry::new();
        let mut run = |from, scope, text| {
            command_search_scoped(&apps, from, scope, &idx, &mut m, &r, &input(text), 5)
        };
        // Both match "code"; the program must not outrank the real app.
        let got = run(1, SearchScope::All, "code");
        assert_eq!(got[0].title, "Code");
        assert_eq!(got[1].payload["program"], true);
        assert!(got[1].score < got[0].score);
        // Empty input lists real apps only.
        assert_eq!(run(1, SearchScope::All, "").len(), 1);
        // Commands scope has no app rows; apps scope keeps them.
        assert!(run(1, SearchScope::Commands, "code").is_empty());
        assert_eq!(run(1, SearchScope::Apps, "code").len(), 2);
    }

    #[test]
    fn program_rows_show_folder_and_gate_admin() {
        let mut p = app("setup");
        p.parsing_name = r"C:\dl\setup.apk".into();
        let it = app_item(&p, 0.5, true);
        assert_eq!(it.subtitle, r"C:\dl");
        assert!(!it.actions.contains(&ActionKind::Admin));
        p.parsing_name = r"C:\dl\setup.EXE".into();
        assert!(app_item(&p, 0.5, true).actions.contains(&ActionKind::Admin));
    }

    async fn live_registry() -> Registry {
        use crate::ctx::{EventSink, FeatureCtx, TaskScope};
        use arc_swap::ArcSwap;
        use launcher_core::Config;
        use launcher_core::store::Store;
        use providers::hub::ProviderHub;
        use std::sync::Arc;
        struct Sink;
        impl EventSink for Sink {
            fn emit(&self, _: &str, _: serde_json::Value) {}
        }
        let c = FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(Config::default())),
            store: Arc::new(Store::open_in_memory().unwrap()),
            providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        };
        let r = Registry::new();
        r.register(Arc::new(crate::syscmd::SysCmd::default())).await;
        r.register(Arc::new(crate::web::Web)).await;
        r.apply_config(&c.config.load(), &c).await;
        r
    }

    fn top(text: &str, r: &Registry) -> Item {
        let apps = vec![app("Clock"), app("Notepad"), app("Calculator")];
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        let idx = MatchIndex::new(&names);
        let mut m = FuzzyMatcher::new();
        command_search(&apps, &idx, &mut m, r, &input(text), 5).remove(0)
    }

    #[tokio::test]
    async fn feature_exact_matches_beat_fuzzy_apps() {
        let r = live_registry().await;
        assert_eq!(top("lock", &r).source, "syscmd");
        assert_eq!(top("g cats", &r).source, "web");
        assert_eq!(top("notepad", &r).title, "Notepad");
    }
}
