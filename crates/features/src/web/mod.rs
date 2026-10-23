//! Web shortcuts: `<keyword> <text>` opens a URL template with `{q}` filled in.
//!
//! A bare keyword with no text yields a hint item with no actions (Enter does nothing, so a
//! half-typed shortcut never launches a browser). State lives in a process-wide slot because the
//! registry constructs `Web` as a unit value; `query` falls back to the defaults before `start`.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{self, Column, Field, FieldKind, SettingsSchema};
use crate::search::fuzzy_pick;
use async_trait::async_trait;
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::RwLock;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Everything except RFC 3986 unreserved characters is escaped; spaces become `%20`.
pub(crate) const QUERY_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Built-in shortcuts: keyword, title, URL template.
const DEFAULTS: [(&str, &str, &str); 6] = [
    ("g", "Google", "https://www.google.com/search?q={q}"),
    (
        "yt",
        "YouTube",
        "https://www.youtube.com/results?search_query={q}",
    ),
    ("gh", "GitHub", "https://github.com/search?q={q}"),
    ("amz", "Amazon", "https://www.amazon.in/s?k={q}"),
    (
        "maps",
        "Google Maps",
        "https://www.google.com/maps/search/{q}",
    ),
    (
        "wiki",
        "Wikipedia",
        "https://en.wikipedia.org/w/index.php?search={q}",
    ),
];

/// Web shortcuts.
pub struct Web;

/// One validated shortcut.
#[derive(Debug, Clone, PartialEq)]
struct Shortcut {
    keyword: String,
    title: String,
    url: String,
    host: String,
}

/// Validated shortcuts plus a lowercase keyword to index map.
#[derive(Debug, Default)]
struct State {
    list: Vec<Shortcut>,
    by_keyword: HashMap<String, usize>,
}

static STATE: RwLock<Option<Arc<State>>> = RwLock::new(None);

fn bad(why: String) -> FeatureError {
    FeatureError::Options(format!("shortcuts: {why}"))
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .to_owned()
}

impl State {
    fn from_rows(rows: &[(String, String, String)]) -> Result<Self, FeatureError> {
        let mut st = State::default();
        for (keyword, title, url) in rows {
            let key = keyword.trim().to_ascii_lowercase();
            if key.is_empty() || key.contains(char::is_whitespace) {
                return Err(bad(format!(
                    "keyword {keyword:?} must be non-empty, no spaces"
                )));
            }
            if st.by_keyword.contains_key(&key) {
                return Err(bad(format!("duplicate keyword {key:?}")));
            }
            let url = url.trim();
            let lower = url.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://"))
                || !url.contains("{q}")
            {
                return Err(bad(format!(
                    "url for {key:?} must be http(s) and contain {{q}}"
                )));
            }
            let title = match title.trim() {
                "" => key.clone(),
                t => t.to_owned(),
            };
            // Store the scheme lowercased so `execute`, which accepts only lowercase http(s),
            // opens an `HTTPS://` template too. The scheme is ASCII, so the split is on a char edge.
            let scheme_end = lower.find("://").unwrap_or(0);
            let url = format!("{}{}", &lower[..scheme_end], &url[scheme_end..]);
            st.by_keyword.insert(key.clone(), st.list.len());
            st.list.push(Shortcut {
                keyword: key,
                title,
                host: host_of(&url),
                url,
            });
        }
        Ok(st)
    }

    fn defaults() -> Self {
        let rows: Vec<_> = DEFAULTS
            .iter()
            .map(|(k, t, u)| ((*k).to_owned(), (*t).to_owned(), (*u).to_owned()))
            .collect();
        // Defaults are constants that pass validation (covered by a test).
        Self::from_rows(&rows).unwrap_or_default()
    }

    /// Reads `shortcuts`; absent means defaults, present replaces them.
    fn from_options(table: &toml::Table) -> Result<Self, FeatureError> {
        schema::validate(&schema_def(), table)?;
        let Some(v) = table.get("shortcuts") else {
            return Ok(Self::defaults());
        };
        let rows: Vec<_> = v
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(toml::Value::as_table)
            .map(|t| {
                let s = |k: &str| {
                    t.get(k)
                        .and_then(toml::Value::as_str)
                        .unwrap_or("")
                        .to_owned()
                };
                (s("keyword"), s("title"), s("url"))
            })
            .collect();
        Self::from_rows(&rows)
    }

    fn fallback_item(&self, text: &str) -> Option<Item> {
        let text = text.trim();
        let first = self.list.first()?;
        if text.is_empty() {
            return None;
        }
        let input = Input {
            raw: String::new(),
            text: format!("{} {text}", first.keyword),
            mode: Mode::Command,
            seq: 0,
        };
        let mut item = self.query(&input).into_iter().next()?;
        item.id = ItemId("web:fallback".into());
        Some(item)
    }

    fn query(&self, input: &Input) -> Vec<Item> {
        if input.mode != Mode::Command {
            return Vec::new();
        }
        let text = input.text.trim();
        let (word, rest) = match text.split_once(char::is_whitespace) {
            Some((w, r)) => (w, r.trim()),
            None => (text, ""),
        };
        let lower = word.to_ascii_lowercase();
        let found = self.by_keyword.get(lower.as_str()).copied().or_else(|| {
            // Typo: keywords of 4+ characters only; short ones would match too much.
            let long = self.list.iter().map(|s| s.keyword.as_str());
            let k = fuzzy_pick(&lower, long.filter(|k| k.chars().count() >= 4))?;
            self.by_keyword.get(k).copied()
        });
        let Some(i) = found else {
            return Vec::new();
        };
        let s = &self.list[i];
        let (title, actions, url) = if rest.is_empty() {
            (
                format!("Type to search {}…", s.title),
                Vec::new(),
                String::new(),
            )
        } else {
            let q = utf8_percent_encode(rest, QUERY_SET).to_string();
            (
                format!("Search {} for “{rest}”", s.title),
                vec![ActionKind::Primary],
                s.url.replace("{q}", &q),
            )
        };
        vec![Item {
            id: ItemId(format!("web:{}", s.keyword)),
            kind: ItemKind::Command,
            title,
            subtitle: s.host.clone(),
            icon: None,
            score: 1.0,
            source: "web".into(),
            actions,
            payload: serde_json::json!({ "url": url }),
        }]
    }
}

fn schema_def() -> SettingsSchema {
    let col = |key: &str, label: &str| Column {
        key: key.into(),
        label: label.into(),
    };
    let rows: Vec<_> = DEFAULTS
        .iter()
        .map(|(k, t, u)| serde_json::json!({ "keyword": k, "title": t, "url": u }))
        .collect();
    SettingsSchema {
        fields: vec![Field {
            key: "shortcuts".into(),
            label: "Shortcuts".into(),
            help: "Keyword, title and an http(s) URL containing {q}. Order is kept.".into(),
            kind: FieldKind::KeyedList {
                columns: vec![
                    col("keyword", "Keyword"),
                    col("title", "Title"),
                    col("url", "URL"),
                ],
            },
            default: serde_json::Value::Array(rows),
            restart: true,
        }],
    }
}

fn current() -> Arc<State> {
    STATE
        .read()
        .clone()
        .unwrap_or_else(|| Arc::new(State::defaults()))
}

/// Title of the first shortcut, used for the plain-text "Search <title>" row. `None` when there are
/// no shortcuts.
pub fn fallback_title() -> Option<String> {
    current().list.first().map(|s| s.title.clone())
}

/// The item `query` builds for `<first keyword> <text>`, with id `web:fallback`. `None` for blank
/// text or no shortcuts. Builds the item only; opening goes through `Web::execute`.
pub fn fallback_item(text: &str) -> Option<Item> {
    current().fallback_item(text)
}

#[async_trait]
impl Feature for Web {
    fn id(&self) -> &'static str {
        "web"
    }
    fn title(&self) -> &'static str {
        "Web shortcuts"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "A keyword plus text opens a URL template with {q}."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        schema_def()
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        let st = State::from_options(&ctx.options("web"))?;
        *STATE.write() = Some(Arc::new(st));
        Ok(())
    }
    fn stop(&self) {
        *STATE.write() = None;
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        current().query(input)
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        let url = item
            .payload
            .get("url")
            .and_then(|u| u.as_str())
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
            .ok_or_else(|| FeatureError::Other("item has no URL".into()))?
            .to_owned();
        // The error never carries the URL: it holds the user's search text.
        tokio::task::spawn_blocking(move || {
            let _com = index_apps::com::ComGuard::new().map_err(|_| "COM init")?;
            index_apps::launch::open_path(Path::new(&url)).map_err(|_| "shell open")
        })
        .await
        .map_err(|e| FeatureError::Other(format!("open task failed: {e}")))?
        .map_err(|kind| FeatureError::Other(format!("Could not open that link ({kind})")))?;
        Ok(Outcome::HideLauncher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> Input {
        Input {
            raw: text.into(),
            text: text.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    fn url(items: &[Item]) -> &str {
        items[0].payload["url"].as_str().unwrap_or("")
    }

    #[test]
    fn fallback_item_encodes_and_skips_blank() {
        let st = State::defaults();
        let Some(it) = st.fallback_item(" a b&c ") else {
            panic!("expected a fallback item");
        };
        assert_eq!(it.id.0, "web:fallback");
        assert_eq!(it.source, "web");
        assert_eq!(it.actions, vec![ActionKind::Primary]);
        assert_eq!(
            it.payload["url"],
            "https://www.google.com/search?q=a%20b%26c"
        );
        assert!(st.fallback_item("   ").is_none());
        assert!(State::default().fallback_item("x").is_none());
    }

    #[test]
    fn fallback_title_is_first_shortcut() {
        // `fallback_title()` reads the process-wide state other tests change, so test the same lookup on
        // owned states.
        assert_eq!(
            State::defaults().list.first().map(|s| s.title.as_str()),
            Some("Google")
        );
        let custom = vec![(
            "ddg".to_owned(),
            "DuckDuckGo".to_owned(),
            "https://duckduckgo.com/?q={q}".to_owned(),
        )];
        let st = State::from_rows(&custom).unwrap_or_default();
        assert_eq!(st.list[0].title, "DuckDuckGo");
    }

    #[test]
    fn six_defaults_in_order() {
        let st = State::defaults();
        let keys: Vec<_> = st.list.iter().map(|s| s.keyword.as_str()).collect();
        assert_eq!(keys, ["g", "yt", "gh", "amz", "maps", "wiki"]);
    }

    #[test]
    fn fuzzy_keyword_of_four_or_more_chars() {
        let st = State::defaults();
        let items = st.query(&input("mpas cats"));
        assert_eq!(items[0].id.0, "web:maps");
        assert!(st.query(&input("gg cats")).is_empty());
    }

    #[test]
    fn encodes_reserved_and_unicode() {
        let items = State::defaults().query(&input("g a b&c=d/é"));
        assert_eq!(
            url(&items),
            "https://www.google.com/search?q=a%20b%26c%3Dd%2F%C3%A9"
        );
        assert_eq!(items[0].title, "Search Google for “a b&c=d/é”");
        assert_eq!(items[0].subtitle, "www.google.com");
    }

    #[test]
    fn unknown_keyword_and_wrong_mode() {
        let st = State::defaults();
        assert!(st.query(&input("zz hello")).is_empty());
        let mut i = input("yt lofi");
        i.mode = Mode::Find;
        assert!(st.query(&i).is_empty());
    }

    #[test]
    fn bare_keyword_is_hint_without_actions() {
        let items = State::defaults().query(&input("yt "));
        assert_eq!(items.len(), 1);
        assert!(items[0].actions.is_empty());
    }

    #[test]
    fn custom_shortcut_from_options() {
        let t: toml::Table = r#"shortcuts = [{ keyword = "DD", title = "Duck", url = "https://duckduckgo.com/?q={q}" }]"#
            .parse()
            .unwrap();
        let st = State::from_options(&t).unwrap();
        let items = st.query(&input("dd rust"));
        assert_eq!(url(&items), "https://duckduckgo.com/?q=rust");
        assert!(st.query(&input("g rust")).is_empty());
    }

    #[test]
    fn uppercase_scheme_is_stored_lowercase() {
        let t: toml::Table = r#"shortcuts = [{ keyword = "d", url = "HTTPS://Duck.com/?Q={q}" }]"#
            .parse()
            .unwrap();
        let st = State::from_options(&t).unwrap();
        // Lowercase scheme, so `execute` accepts it; the rest of the template is untouched.
        assert_eq!(url(&st.query(&input("d x"))), "https://Duck.com/?Q=x");
    }

    #[test]
    fn invalid_options_rejected() {
        for body in [
            r#"{ keyword = "", url = "https://x/{q}" }"#,
            r#"{ keyword = "a b", url = "https://x/{q}" }"#,
            r#"{ keyword = "a", url = "ftp://x/{q}" }"#,
            r#"{ keyword = "a", url = "https://x/" }"#,
            r#"{ keyword = "a", url = "https://x/{q}" }, { keyword = "A", url = "https://y/{q}" }"#,
        ] {
            let t: toml::Table = format!("shortcuts = [{body}]").parse().unwrap();
            assert!(State::from_options(&t).is_err(), "{body}");
        }
    }
}
