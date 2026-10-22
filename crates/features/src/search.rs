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
const PROGRAM_FACTOR: f32 = 0.95;
/// Program extensions that can run elevated.
const ADMIN_EXTS: &[&str] = &["exe", "msi"];
/// Nucleo scores are unbounded; feature items score 0..1. Squash apps into 0..=APP_CEIL so an
/// exact feature keyword match (1.0) outranks a fuzzy app hit, while apps still order among themselves.
/// Scores are divided by max(perfect score of the query, best hit), one absolute scale for apps and
/// programs: a scattered app hit cannot outrank a contiguous program hit just by being the best app.
/// ponytail: nucleo's self-score is the ceiling; if a hit exceeds it the best hit becomes the ceiling.
const APP_CEIL: f32 = 0.9;

/// Whether `a` may run elevated. `program` says it came from the programs scan: only exe and msi
/// files do. Real apps (Start menu, packaged, Desktop shortcuts) always may. Used by both the row and `launch`.
pub fn can_run_as_admin(a: &AppEntry, program: bool) -> bool {
    if !program {
        return true;
    }
    std::path::Path::new(&a.parsing_name)
        .extension()
        .is_some_and(|e| ADMIN_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// `idx` is the entry's position in its own list (apps or programs); it rides in the payload as `entry`.
/// `removable` (programs only) marks a program on a removable drive: labelled, no Admin action.
fn app_item(a: &AppEntry, idx: usize, score: f32, program: bool, removable: bool) -> Item {
    let mut actions = vec![ActionKind::Primary, ActionKind::Location];
    let mut payload =
        serde_json::json!({ "appId": a.id, "parsingName": a.parsing_name, "entry": idx });
    let mut subtitle = String::new();
    if program {
        subtitle = std::path::Path::new(&a.parsing_name)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        payload["program"] = serde_json::json!(true);
        if removable {
            subtitle = format!("Removable · {subtitle}");
            payload["removable"] = serde_json::json!(true);
        }
    }
    if can_run_as_admin(a, program) && !removable {
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

fn top_k_sorted(mut items: Vec<Item>, k: usize) -> Vec<Item> {
    if k > 0 && items.len() > k {
        items.select_nth_unstable_by(k - 1, by_score);
    }
    items.truncate(k);
    items.sort_by(by_score);
    items
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
        SearchScope::All,
        app_index,
        matcher,
        registry,
        input,
        k,
    )
}

/// Command-mode results: real apps (fuzzy, or most-used when `input.text` is empty) plus feature items,
/// ranked with feature boosts, at most `k`. Scanned programs are matched separately by [`program_items`]
/// (their index is much larger) and joined with [`merge_items`].
pub fn command_search_scoped(
    apps: &[AppEntry],
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
                    .enumerate()
                    .map(|(i, a)| app_item(a, i, 0.0, false, false)),
            );
        } else {
            let hits = matcher.top_k(&input.text, app_index, k);
            let best = hits.iter().map(|h| h.score as f32).fold(0.0_f32, f32::max);
            let max = best.max(matcher.perfect_score(&input.text) as f32);
            items.extend(hits.into_iter().filter_map(|h| {
                let a = apps.get(h.idx)?;
                let score = if max > 0.0 {
                    APP_CEIL * h.score as f32 / max
                } else {
                    0.0
                };
                Some(app_item(a, h.idx, score, false, false))
            }));
        }
    }
    if scope != SearchScope::Apps {
        items.extend(registry.query_all(input));
    }
    // Boost always runs: usage history orders apps too (and empty "!" lists most-used first).
    for it in &mut items {
        it.score += registry.rank_boost_all(input, it);
    }
    top_k_sorted(items, k)
}

/// Whether a program name is a solid match: it contains the query, or every query word starts a name word.
/// Scattered fuzzy hits (letters spread across a long file name) fail both.
fn solid_match(name: &str, query: &str) -> bool {
    let (name, query) = (name.to_lowercase(), query.trim().to_lowercase());
    name.contains(&query)
        || query.split_whitespace().all(|q| {
            name.split(|c: char| !c.is_alphanumeric())
                .any(|w| w.starts_with(q))
        })
}

/// Fuzzy matches of `text` over scanned programs, best `k`, scored below real apps. Empty text matches nothing.
/// `removable[i]` says program `i` sits on a removable drive (missing entries count as fixed).
/// Budget is Find's 25 ms; call from a blocking thread.
pub fn program_items(
    programs: &[AppEntry],
    removable: &[bool],
    index: &MatchIndex,
    matcher: &mut FuzzyMatcher,
    text: &str,
    k: usize,
) -> Vec<Item> {
    if text.is_empty() || k == 0 {
        return Vec::new();
    }
    let hits = matcher.top_k(text, index, k);
    let best = hits.iter().map(|h| h.score as f32).fold(0.0_f32, f32::max);
    if best <= 0.0 {
        return Vec::new();
    }
    let max = best.max(matcher.perfect_score(text) as f32);
    hits.into_iter()
        .filter_map(|h| {
            let a = programs.get(h.idx)?;
            // Weak hits (< half the best score, or scattered letters) are noise.
            if (h.score as f32) < best * 0.5 || !solid_match(&a.name, text) {
                return None;
            }
            Some(app_item(
                a,
                h.idx,
                APP_CEIL * PROGRAM_FACTOR * h.score as f32 / max,
                true,
                removable.get(h.idx).copied().unwrap_or(false),
            ))
        })
        .collect()
}

/// Joins [`command_search_scoped`] and [`program_items`] results for query `text`, at most `k`.
/// A program never outranks an installed app that solidly matches the same query: its score is
/// capped just below that app's, so a file named like an app (chrome.exe) cannot hijack Enter.
pub fn merge_items(
    mut items: Vec<Item>,
    mut programs: Vec<Item>,
    text: &str,
    k: usize,
) -> Vec<Item> {
    let best_app = items
        .iter()
        .filter(|i| {
            i.source == "apps" && i.payload.get("program").is_none() && solid_match(&i.title, text)
        })
        .map(|i| i.score)
        .reduce(f32::max);
    if let Some(best) = best_app {
        let cap = (best - 0.01).max(0.0);
        for p in &mut programs {
            p.score = p.score.min(cap);
        }
    }
    items.extend(programs);
    top_k_sorted(items, k)
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

    fn idx_of(list: &[AppEntry]) -> MatchIndex {
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        MatchIndex::new(&names)
    }

    #[test]
    fn programs_rank_below_apps_and_scope_filters() {
        let apps = vec![app("Code")];
        let progs = vec![app("Code Helper")];
        let (ai, pi) = (idx_of(&apps), idx_of(&progs));
        let mut m = FuzzyMatcher::new();
        let r = Registry::new();
        let mut run = |scope, text: &str| {
            let a = command_search_scoped(&apps, scope, &ai, &mut m, &r, &input(text), 5);
            let p = if scope == SearchScope::Commands {
                Vec::new()
            } else {
                program_items(&progs, &[], &pi, &mut FuzzyMatcher::new(), text, 5)
            };
            merge_items(a, p, text, 5)
        };
        // Both match "code"; the program must not outrank the real app.
        let got = run(SearchScope::All, "code");
        assert_eq!(got[0].title, "Code");
        assert_eq!(got[1].payload["program"], true);
        assert_eq!(got[1].payload["entry"], 0);
        assert!(got[1].score < got[0].score);
        // Empty input lists real apps only.
        assert_eq!(run(SearchScope::All, "").len(), 1);
        assert!(run(SearchScope::Commands, "code").is_empty());
        assert_eq!(run(SearchScope::Apps, "code").len(), 2);
    }

    #[test]
    fn contiguous_program_beats_scattered_app() {
        let apps = vec![app("Windows Media Player Legacy")];
        let progs = vec![app("Inlay-2.0.0")];
        let (ai, pi) = (idx_of(&apps), idx_of(&progs));
        let mut m = FuzzyMatcher::new();
        let a = command_search_scoped(
            &apps,
            SearchScope::All,
            &ai,
            &mut m,
            &Registry::new(),
            &input("inlay"),
            5,
        );
        let p = program_items(&progs, &[], &pi, &mut m, "inlay", 5);
        let got = merge_items(a, p, "inlay", 5);
        assert_eq!(got[0].title, "Inlay-2.0.0");
    }

    #[test]
    fn program_named_like_an_app_never_outranks_it() {
        let apps = vec![app("Google Chrome")];
        let progs = vec![app("chrome")];
        let (ai, pi) = (idx_of(&apps), idx_of(&progs));
        let mut m = FuzzyMatcher::new();
        let a = command_search_scoped(
            &apps,
            SearchScope::All,
            &ai,
            &mut m,
            &Registry::new(),
            &input("chrome"),
            5,
        );
        let p = program_items(&progs, &[true], &pi, &mut m, "chrome", 5);
        assert_eq!(p.len(), 1);
        let got = merge_items(a, p, "chrome", 5);
        assert_eq!(got[0].title, "Google Chrome");
        assert_eq!(got[1].payload["program"], true);
        assert!(got[1].score < got[0].score);
    }

    #[test]
    fn removable_program_is_labelled_without_admin() {
        let mut p = app("setup");
        p.parsing_name = r"E:\dl\setup.exe".into();
        let it = app_item(&p, 0, 0.5, true, true);
        assert_eq!(it.subtitle, r"Removable · E:\dl");
        assert_eq!(it.payload["removable"], true);
        assert!(!it.actions.contains(&ActionKind::Admin));
        let fixed = app_item(&p, 0, 0.5, true, false);
        assert_eq!(fixed.subtitle, r"E:\dl");
        assert!(fixed.payload.get("removable").is_none());
        assert!(fixed.actions.contains(&ActionKind::Admin));
    }

    #[test]
    fn scattered_program_hits_are_dropped() {
        let progs = vec![
            app("Spotify"),
            app("snapshot-bash-1700000000000-i8kfky"),
            app("Visual Studio Code"),
        ];
        let pi = idx_of(&progs);
        let got = program_items(&progs, &[], &pi, &mut FuzzyMatcher::new(), "spotify", 5);
        let titles: Vec<_> = got.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["Spotify"]);
        assert!(solid_match("Visual Studio Code", "studio co"));
        assert!(!solid_match("snapshot-bash-i8kfky", "spotify"));
    }

    #[test]
    fn merge_keeps_best_k_across_both_lists() {
        let mk = |t: &str, s: f32| app_item(&app(t), 0, s, false, false);
        let got = merge_items(
            vec![mk("a", 0.9), mk("b", 0.2)],
            vec![mk("c", 0.6)],
            "zz",
            2,
        );
        let titles: Vec<_> = got.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["a", "c"]);
    }

    #[test]
    fn program_rows_show_folder_and_gate_admin() {
        let mut p = app("setup");
        p.parsing_name = r"C:\dl\setup.apk".into();
        let it = app_item(&p, 0, 0.5, true, false);
        assert_eq!(it.subtitle, r"C:\dl");
        assert!(!it.actions.contains(&ActionKind::Admin));
        p.parsing_name = r"C:\dl\setup.EXE".into();
        assert!(
            app_item(&p, 0, 0.5, true, false)
                .actions
                .contains(&ActionKind::Admin)
        );
        // A real app with an absolute .lnk path (Desktop extra) keeps Admin; the same path as a program does not.
        p.parsing_name = r"C:\Users\me\Desktop\x.lnk".into();
        assert!(can_run_as_admin(&p, false));
        assert!(!can_run_as_admin(&p, true));
        assert!(
            app_item(&p, 0, 0.5, false, false)
                .actions
                .contains(&ActionKind::Admin)
        );
    }

    #[tokio::test]
    async fn chosen_app_outranks_in_apps_scope() {
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
        r.register(Arc::new(crate::frecency::Frecency::default()))
            .await;
        r.apply_config(&c.config.load(), &c).await;
        let apps = vec![app("Alpha"), app("Bravo")];
        let ai = idx_of(&apps);
        let mut m = FuzzyMatcher::new();
        for text in ["", "a"] {
            r.on_chosen_all(&input(text), &app_item(&apps[1], 1, 0.0, false, false));
            let got =
                command_search_scoped(&apps, SearchScope::Apps, &ai, &mut m, &r, &input(text), 5);
            assert_eq!(got[0].title, "Bravo", "text {text:?}");
        }
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
