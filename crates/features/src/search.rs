//! Search composition for Command and Find modes.

use crate::registry::{FeatureSel, Registry};
use index_apps::AppEntry;
use index_files::{FileHit, FileIndex, FindQuery, Overlay};
use launcher_core::matcher::{FuzzyMatcher, MatchIndex, keyword_score, one_edit};
use launcher_core::{ActionKind, Input, Item, ItemId, ItemKind};
use std::cmp::Ordering;

/// Which sources a search draws from (wire ids: see [`Sources::parse`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// Installed apps and scanned programs.
    pub apps: bool,
    /// The file index.
    pub files: bool,
    /// Which running features.
    pub features: FeatureSel,
}

impl Sources {
    /// Apps and every feature, no files: what Command mode searched before sources existed.
    pub fn everything() -> Self {
        Self {
            apps: true,
            files: false,
            features: FeatureSel::All,
        }
    }

    /// Parses wire ids. `"apps"`, `"files"` and `"features"` (all features) are reserved; any other id
    /// must satisfy `known` (a feature id). Duplicates collapse; an empty list selects nothing.
    /// The error is the first unknown id.
    pub fn parse(ids: &[String], known: impl Fn(&str) -> bool) -> Result<Sources, String> {
        let mut s = Sources::default();
        let mut some: Vec<String> = Vec::new();
        let mut all = false;
        for id in ids {
            match id.as_str() {
                "apps" => s.apps = true,
                "files" => s.files = true,
                "features" => all = true,
                f if known(f) => {
                    if !some.iter().any(|x| x == f) {
                        some.push(f.to_owned());
                    }
                }
                _ => return Err(id.clone()),
            }
        }
        s.features = if all {
            FeatureSel::All
        } else if some.is_empty() {
            FeatureSel::None
        } else {
            FeatureSel::Ids(some)
        };
        Ok(s)
    }
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

/// Typo fallback for apps when nucleo found nothing: a single query word of 4+ characters within one
/// edit of any word of an app name scores `0.3 * APP_CEIL`. At most `k` rows.
fn one_edit_apps(apps: &[AppEntry], text: &str, k: usize) -> Vec<Item> {
    let q = text.trim();
    if q.chars().count() < 4 || q.contains(char::is_whitespace) {
        return Vec::new();
    }
    apps.iter()
        .enumerate()
        .filter(|(_, a)| {
            a.name
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| one_edit(q, w))
        })
        .take(k)
        .map(|(i, a)| app_item(a, i, 0.3 * APP_CEIL, false, false))
        .collect()
}

/// Picks the keyword a typed word means: the word itself when it is one, else the single keyword
/// it is a fuzzy (subsequence or one-edit) match for. Prefixes are not accepted, so the fuzzy
/// path never changes what correct or half-typed input does. Words under 4 characters match nothing;
/// a tie between keywords matches nothing.
pub(crate) fn fuzzy_pick<'a>(
    word: &str,
    keywords: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    if word.chars().count() < 4 {
        return None;
    }
    let mut best: Option<(f32, &str)> = None;
    let mut tie = false;
    for k in keywords {
        let Some(s) = keyword_score(word, k).filter(|s| *s >= 1.0 || *s < 0.5) else {
            continue;
        };
        match best {
            Some((b, _)) if s < b => {}
            Some((b, _)) if s == b => tie = true,
            _ => {
                best = Some((s, k));
                tie = false;
            }
        }
    }
    best.filter(|_| !tie).map(|(_, k)| k)
}

/// Command-mode results over apps and all features; see [`command_search_scoped`].
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
        &Sources::everything(),
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
    sources: &Sources,
    app_index: &MatchIndex,
    matcher: &mut FuzzyMatcher,
    registry: &Registry,
    input: &Input,
    k: usize,
) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    if sources.apps {
        if input.text.is_empty() {
            items.extend(
                apps.iter()
                    .enumerate()
                    .map(|(i, a)| app_item(a, i, 0.0, false, false)),
            );
        } else {
            let hits = matcher.top_k(&input.text, app_index, k);
            if hits.is_empty() {
                items.extend(one_edit_apps(apps, &input.text, k));
            }
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
    items.extend(registry.query_sel(input, &sources.features));
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

/// Display name of a program: its file name without the extension.
fn program_name(path: &str) -> std::borrow::Cow<'_, str> {
    std::path::Path::new(path)
        .file_stem()
        .map_or(std::borrow::Cow::Borrowed(""), |s| s.to_string_lossy())
}

/// The [`AppEntry`] of a scanned program path, built on demand (programs are stored as bare paths).
/// The shell path is the parsing name; `target` stays `None`.
pub fn program_entry(path: &str) -> AppEntry {
    AppEntry {
        id: index_apps::make_id(path),
        name: program_name(path).into_owned(),
        parsing_name: path.to_owned(),
        target: None,
        is_packaged: false,
    }
}

/// Fuzzy index over the file names of `paths`, same order.
pub fn program_index(paths: &[Box<str>]) -> MatchIndex {
    let names: Vec<std::borrow::Cow<'_, str>> = paths.iter().map(|p| program_name(p)).collect();
    MatchIndex::new(&names)
}

/// Fuzzy matches of `text` over scanned program `paths`, best `k`, scored below real apps. Empty text
/// matches nothing. `removable[i]` says program `i` sits on a removable drive (missing entries count
/// as fixed). Entries are built only for the hits. Budget is Find's 25 ms; call from a blocking thread.
pub fn program_items(
    programs: &[Box<str>],
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
            let a = program_entry(programs.get(h.idx)?);
            // Weak hits (< half the best score, or scattered letters) are noise.
            if (h.score as f32) < best * 0.5 || !solid_match(&a.name, text) {
                return None;
            }
            Some(app_item(
                &a,
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

/// The `/` list (C5): feature rows (the natural-language finder), apps, programs, files, folders, at
/// most `k`. Up to 3 rows are reserved for files when any exist; unused room goes to the other side.
pub fn compose_open(cmd_items: Vec<Item>, files: Vec<Item>, k: usize) -> Vec<Item> {
    let (apps, mut rows): (Vec<Item>, Vec<Item>) =
        cmd_items.into_iter().partition(|i| i.source == "apps");
    let (programs, apps): (Vec<Item>, Vec<Item>) = apps
        .into_iter()
        .partition(|i| i.payload.get("program").is_some());
    let (files, folders): (Vec<Item>, Vec<Item>) =
        files.into_iter().partition(|i| i.kind == ItemKind::File);
    rows.truncate(k);
    let room = k - rows.len();
    let nfiles = files.len() + folders.len();
    let nrun = apps.len() + programs.len();
    let run_take = nrun.min(room - nfiles.min(3).min(room));
    let file_take = nfiles.min(room - run_take);
    rows.extend(apps.into_iter().chain(programs).take(run_take));
    rows.extend(files.into_iter().chain(folders).take(file_take));
    rows
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

    const APPS: Sources = Sources {
        apps: true,
        files: false,
        features: FeatureSel::None,
    };
    const FEATURES: Sources = Sources {
        apps: false,
        files: false,
        features: FeatureSel::All,
    };

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
    fn typo_finds_spotify_app() {
        let apps = vec![app("Spotify"), app("Notepad")];
        let ai = idx_of(&apps);
        let mut m = FuzzyMatcher::new();
        let r = Registry::new();
        for q in ["spotfy", "spotifu"] {
            let got = command_search_scoped(&apps, &APPS, &ai, &mut m, &r, &input(q), 5);
            assert_eq!(
                got.first().map(|i| i.title.as_str()),
                Some("Spotify"),
                "{q}"
            );
        }
    }

    #[test]
    fn sources_parse() {
        let known = |id: &str| id == "math" || id == "web";
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let s = Sources::parse(&ids(&["apps", "files", "math", "web", "math"]), known).unwrap();
        assert!(s.apps && s.files);
        assert_eq!(
            s.features,
            FeatureSel::Ids(vec!["math".into(), "web".into()])
        );
        let s = Sources::parse(&ids(&["apps", "features", "math"]), known).unwrap();
        assert_eq!(s.features, FeatureSel::All);
        assert_eq!(
            Sources::parse(&ids(&["apps", "nope", "bad"]), known),
            Err("nope".into())
        );
        assert_eq!(Sources::parse(&[], known), Ok(Sources::default()));
        assert_eq!(Sources::parse(&ids(&["ask"]), known), Err("ask".into()));
    }

    fn row(id: &str, kind: ItemKind, source: &str, program: bool) -> Item {
        Item {
            id: ItemId(id.into()),
            kind,
            title: id.into(),
            subtitle: String::new(),
            icon: None,
            score: 0.0,
            source: source.into(),
            actions: vec![],
            payload: if program {
                serde_json::json!({ "program": true })
            } else {
                serde_json::Value::Null
            },
        }
    }

    fn ids_of(v: &[Item]) -> Vec<&str> {
        v.iter().map(|i| i.id.0.as_str()).collect()
    }

    #[test]
    fn compose_open_order() {
        let a = |n: &str| row(n, ItemKind::App, "apps", false);
        let p = |n: &str| row(n, ItemKind::App, "apps", true);
        let f = |n: &str| row(n, ItemKind::File, "files", false);
        let d = |n: &str| row(n, ItemKind::Folder, "files", false);
        let nl = row("nl", ItemKind::Result, "nlfind", false);
        // Order: feature row, apps, programs, files, folders (inputs deliberately shuffled).
        let got = compose_open(
            vec![p("p1"), a("a1"), nl.clone()],
            vec![d("d1"), f("f1")],
            8,
        );
        assert_eq!(ids_of(&got), ["nl", "a1", "p1", "f1", "d1"]);
        // Plentiful apps: 3 rows stay reserved for files, 1 row for the feature.
        let apps: Vec<Item> = (0..8).map(|i| a(&format!("a{i}"))).collect();
        let files: Vec<Item> = (0..5).map(|i| f(&format!("f{i}"))).collect();
        let mut cmd = apps.clone();
        cmd.push(nl);
        let got = compose_open(cmd, files.clone(), 8);
        assert_eq!(
            ids_of(&got),
            ["nl", "a0", "a1", "a2", "a3", "f0", "f1", "f2"]
        );
        // No files: apps fill the room, k respected.
        assert_eq!(compose_open(apps.clone(), Vec::new(), 8).len(), 8);
        // One file only: it takes one row, apps the rest.
        let got = compose_open(apps, files[..1].to_vec(), 8);
        assert_eq!(got.len(), 8);
        assert_eq!(got[7].id.0, "f0");
        // Few apps: files take the leftover room.
        let got = compose_open(vec![a("a0")], files, 4);
        assert_eq!(ids_of(&got), ["a0", "f0", "f1", "f2"]);
        assert!(compose_open(Vec::new(), Vec::new(), 8).is_empty());
    }

    #[test]
    fn fuzzy_pick_rules() {
        let kws = ["port", "proc"];
        assert_eq!(fuzzy_pick("port", kws), Some("port"));
        assert_eq!(fuzzy_pick("prot", kws), None);
        assert_eq!(fuzzy_pick("potr", kws), Some("port"));
        assert_eq!(fuzzy_pick("por", kws), None);
    }

    fn idx_of(list: &[AppEntry]) -> MatchIndex {
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        MatchIndex::new(&names)
    }

    /// Program paths `C:\x\<name>.exe`.
    fn paths(names: &[&str]) -> Vec<Box<str>> {
        names
            .iter()
            .map(|n| format!(r"C:\x\{n}.exe").into_boxed_str())
            .collect()
    }

    #[test]
    fn program_entry_matches_the_old_entry_shape() {
        let e = program_entry(r"C:\x\Setup Tool.EXE");
        assert_eq!(e.name, "Setup Tool");
        assert_eq!(e.parsing_name, r"C:\x\Setup Tool.EXE");
        assert_eq!(e.id, index_apps::make_id(r"C:\x\Setup Tool.EXE"));
        assert!(e.target.is_none() && !e.is_packaged);
        assert!(can_run_as_admin(&e, true));
        assert!(!can_run_as_admin(&program_entry(r"C:\x\a.lnk"), true));
    }

    #[test]
    fn programs_rank_below_apps_and_sources_filter() {
        let apps = vec![app("Code")];
        let progs = paths(&["Code Helper"]);
        let (ai, pi) = (idx_of(&apps), program_index(&progs));
        let mut m = FuzzyMatcher::new();
        let r = Registry::new();
        let every = Sources::everything();
        let mut run = |src: &Sources, text: &str| {
            let a = command_search_scoped(&apps, src, &ai, &mut m, &r, &input(text), 5);
            let p = if !src.apps {
                Vec::new()
            } else {
                program_items(&progs, &[], &pi, &mut FuzzyMatcher::new(), text, 5)
            };
            merge_items(a, p, text, 5)
        };
        // Both match "code"; the program must not outrank the real app.
        let got = run(&every, "code");
        assert_eq!(got[0].title, "Code");
        assert_eq!(got[1].payload["program"], true);
        assert_eq!(got[1].payload["entry"], 0);
        assert!(got[1].score < got[0].score);
        // Empty input lists real apps only.
        assert_eq!(run(&every, "").len(), 1);
        assert!(run(&FEATURES, "code").is_empty());
        assert_eq!(run(&APPS, "code").len(), 2);
    }

    #[test]
    fn contiguous_program_beats_scattered_app() {
        let apps = vec![app("Windows Media Player Legacy")];
        let progs = paths(&["Inlay-2.0.0"]);
        let (ai, pi) = (idx_of(&apps), program_index(&progs));
        let mut m = FuzzyMatcher::new();
        let a = command_search_scoped(
            &apps,
            &Sources::everything(),
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
        let progs = paths(&["chrome"]);
        let (ai, pi) = (idx_of(&apps), program_index(&progs));
        let mut m = FuzzyMatcher::new();
        let a = command_search_scoped(
            &apps,
            &Sources::everything(),
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
        let progs = paths(&[
            "Spotify",
            "snapshot-bash-1700000000000-i8kfky",
            "Visual Studio Code",
        ]);
        let pi = program_index(&progs);
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
            let got = command_search_scoped(&apps, &APPS, &ai, &mut m, &r, &input(text), 5);
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
