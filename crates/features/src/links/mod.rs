//! Links and paths: a typed http(s) URL or an absolute Windows path gets one Open row.
//!
//! `query` only parses text; it never touches the filesystem, the environment or the network.
//! The `%NAME%` and `~` expansions read a small allow-list of variables captured in `start`.
//! Opening happens only in `execute`, which re-validates the target and, for a path, checks that
//! it exists first. Targets go to the shell-open helper as a file name, never as a command line.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;

/// Longest text that is classified at all.
const MAX_LEN: usize = 2048;

/// Variables `%NAME%` may name; anything else is not expanded and the text is not a path.
const ALLOWED_VARS: [&str; 6] = [
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "TEMP",
    "PROGRAMDATA",
    "PROGRAMFILES",
];

/// Top-level domains a bare domain may end in. File extensions that are also TLDs (`md`, `py`,
/// `rs`, `sh`, `pl`, `zip`, `mov`, ...) are left out on purpose so file names are not links.
const TLDS: [&str; 23] = [
    "com", "org", "net", "io", "dev", "app", "ai", "co", "in", "uk", "de", "fr", "us", "edu",
    "gov", "info", "me", "tv", "xyz", "tech", "cloud", "site", "online",
];

/// Characters never allowed in a path: Windows forbids them in names, and `"` would break any
/// quoting downstream.
const BAD_PATH_CHARS: [char; 6] = ['<', '>', '"', '|', '?', '*'];

/// Allow-listed variables, uppercase name to value, captured in `start`.
type Vars = HashMap<&'static str, String>;

/// What typed text points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An `http` or `https` URL, normalised.
    Url(String),
    /// An absolute drive path, with any variable expanded.
    Path(String),
}

impl Target {
    fn as_str(&self) -> &str {
        match self {
            Target::Url(s) | Target::Path(s) => s,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Target::Url(_) => "url",
            Target::Path(_) => "path",
        }
    }
}

/// Typed links and paths.
#[derive(Default)]
pub struct Links {
    vars: RwLock<Vars>,
}

/// Reads the allow-listed variables. Unset or non-UTF-8 values are left out.
fn read_vars() -> Vars {
    ALLOWED_VARS
        .iter()
        .filter_map(|&k| {
            let v = std::env::var_os(k)?.into_string().ok()?;
            Some((k, v))
        })
        .collect()
}

/// Classifies `text` as a URL or a path. Pure: no I/O; variables come from `vars`.
pub fn classify(text: &str, vars: &HashMap<&'static str, String>) -> Option<Target> {
    // Control characters (newlines included) are checked before trimming so they cannot hide.
    if text.chars().any(char::is_control) {
        return None;
    }
    let t = text.trim();
    if t.is_empty() || t.len() > MAX_LEN {
        return None;
    }
    classify_path(t, vars)
        .map(Target::Path)
        .or_else(|| classify_url(t).map(Target::Url))
}

fn classify_url(t: &str) -> Option<String> {
    if t.chars().any(char::is_whitespace) || t.contains('\\') {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let full = if lower.starts_with("http://") || lower.starts_with("https://") {
        t.to_owned()
    } else if is_bare_domain(t) {
        format!("https://{t}")
    } else {
        return None;
    };
    let url = url::Url::parse(&full).ok()?;
    let ok = matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|h| !h.is_empty())
        // `https://bank.com@evil.com` hides the real host; never offer it.
        && url.username().is_empty()
        && url.password().is_none();
    ok.then(|| url.as_str().to_owned())
}

/// `host[.host]*.tld[:port][/path][?query][#frag]` with `tld` in [`TLDS`].
fn is_bare_domain(t: &str) -> bool {
    let end = t.find(['/', '?', '#']).unwrap_or(t.len());
    let authority = &t[..end];
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    let port_ok =
        port.is_none_or(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()));
    let labels: Vec<&str> = host.split('.').collect();
    let label_ok = |l: &&str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    port_ok
        && labels.len() >= 2
        && labels.iter().all(label_ok)
        && labels
            .last()
            .is_some_and(|tld| TLDS.contains(&tld.to_ascii_lowercase().as_str()))
}

fn is_drive_abs(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')
}

fn classify_path(t: &str, vars: &Vars) -> Option<String> {
    let expanded = if let Some(rest) = t.strip_prefix('%') {
        let (name, tail) = rest.split_once('%')?;
        let upper = name.to_ascii_uppercase();
        let value = vars.get(upper.as_str())?;
        format!("{value}{tail}")
    } else if let Some(tail) = t.strip_prefix("~\\").or_else(|| t.strip_prefix("~/")) {
        format!("{}\\{tail}", vars.get("USERPROFILE")?)
    } else {
        t.to_owned()
    };
    // Drive paths only: this also rejects UNC (`\\server`), device paths (`\\?\`, `\\.\`) and
    // drive-relative `C:foo`, including when they came out of a variable.
    if !is_drive_abs(&expanded)
        || expanded.contains(BAD_PATH_CHARS)
        // A second `:` is an alternate data stream.
        || expanded[2..].contains(':')
        || expanded.split(['\\', '/']).any(|c| c == "..")
    {
        return None;
    }
    Some(expanded)
}

impl Links {
    fn classify(&self, text: &str) -> Option<Target> {
        classify(text, &self.vars.read())
    }
}

#[async_trait]
impl Feature for Links {
    fn id(&self) -> &'static str {
        "links"
    }
    fn title(&self) -> &'static str {
        "Links and paths"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Typed links and paths get an Open row."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema::default()
    }
    fn start(&self, _ctx: FeatureCtx) -> Result<(), FeatureError> {
        *self.vars.write() = read_vars();
        Ok(())
    }
    fn stop(&self) {
        self.vars.write().clear();
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let Some(target) = self.classify(&input.text) else {
            return Vec::new();
        };
        let s = target.as_str();
        vec![Item {
            id: ItemId(format!("links:{s}")),
            kind: ItemKind::Command,
            title: format!("Open {s}"),
            subtitle: match target {
                Target::Url(_) => "Link".into(),
                Target::Path(_) => "Path".into(),
            },
            icon: None,
            score: 1.0,
            source: "links".into(),
            actions: vec![ActionKind::Primary],
            payload: serde_json::json!({ "target": s, "type": target.kind() }),
        }]
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        // Never trust the payload: classify it again and require the same type.
        let raw = item.payload.get("target").and_then(|t| t.as_str());
        let kind = item.payload.get("type").and_then(|t| t.as_str());
        let target = raw
            .and_then(|r| self.classify(r))
            .filter(|t| Some(t.kind()) == kind)
            .ok_or_else(|| FeatureError::Other("not a link or path".into()))?;
        let opened = tokio::task::spawn_blocking(move || match target {
            Target::Url(u) => index_apps::launch::open_path(Path::new(&u))
                .map_err(|_| format!("could not open {u}")),
            Target::Path(p) => {
                let path = Path::new(&p);
                if !path.exists() {
                    return Err("That path does not exist".to_owned());
                }
                index_apps::launch::open_path(path).map_err(|_| format!("could not open {p}"))
            }
        })
        .await
        .map_err(|e| FeatureError::Other(format!("open task failed: {e}")))?;
        opened.map_err(FeatureError::Other)?;
        Ok(Outcome::HideLauncher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        HashMap::from([
            ("APPDATA", r"C:\Users\me\AppData\Roaming".to_owned()),
            ("USERPROFILE", r"C:\Users\me".to_owned()),
            ("TEMP", r"\\server\tmp".to_owned()),
        ])
    }

    fn c(text: &str) -> Option<Target> {
        classify(text, &vars())
    }

    fn url(s: &str) -> Option<Target> {
        Some(Target::Url(s.into()))
    }

    fn path(s: &str) -> Option<Target> {
        Some(Target::Path(s.into()))
    }

    fn input(text: &str) -> Input {
        Input {
            raw: text.into(),
            text: text.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    fn links() -> Links {
        Links {
            vars: RwLock::new(vars()),
        }
    }

    #[test]
    fn accepts() {
        let cases = [
            ("https://github.com/x", url("https://github.com/x")),
            ("HTTP://Example.com", url("http://example.com/")),
            ("github.com/x", url("https://github.com/x")),
            ("example.org", url("https://example.org/")),
            ("  example.org  ", url("https://example.org/")),
            (
                "docs.rs.dev:8080/a?b=c",
                url("https://docs.rs.dev:8080/a?b=c"),
            ),
            ("http://localhost:3000", url("http://localhost:3000/")),
            (r"C:\Users\me\Docs", path(r"C:\Users\me\Docs")),
            ("C:/tmp/a", path("C:/tmp/a")),
            (r"d:\Program Files\x", path(r"d:\Program Files\x")),
            (r"%APPDATA%\Foo", path(r"C:\Users\me\AppData\Roaming\Foo")),
            (r"%appdata%\Foo", path(r"C:\Users\me\AppData\Roaming\Foo")),
            (r"~\Downloads", path(r"C:\Users\me\Downloads")),
            ("~/Downloads", path(r"C:\Users\me\Downloads")),
        ];
        for (text, want) in cases {
            assert_eq!(c(text), want, "{text}");
        }
    }

    #[test]
    fn rejects() {
        let long = format!("https://example.com/{}", "a".repeat(3000));
        let cases = [
            "",
            "   ",
            "localhost:3000",
            "notes.md",
            "main.rs",
            "readme.txt",
            "a.zip",
            "script.py",
            r"\\server\share",
            r"\\?\C:\x",
            r"\\.\PhysicalDrive0",
            r"C:\a\..\b",
            "C:/a/../b",
            r"C:\..",
            "C:foo",
            r"C:\a:stream",
            r#"C:\a"b"#,
            "C:\\a\nb",
            "C:\\a\tb",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "file:///c:/x",
            "data:text/html,hi",
            "ms-settings:",
            "ms-settings:display",
            "ftp://x.com",
            "two words.com",
            "https://exa mple.com",
            "https://a.com\n",
            "https://bank.com@evil.com",
            "bank.com@evil.com",
            "https://",
            "example.com:",
            "example.com:123456",
            "-bad.com",
            ".com",
            r"example.com\x",
            r"%TEMP%\x",
            r"%WINDIR%\x",
            r"%PATH%",
            r"%APPDATA",
            &long,
        ];
        for text in cases {
            assert_eq!(c(text), None, "{text:?}");
        }
    }

    #[test]
    fn home_needs_userprofile() {
        assert_eq!(classify(r"~\Downloads", &HashMap::new()), None);
        assert_eq!(classify(r"%APPDATA%\x", &HashMap::new()), None);
    }

    #[test]
    fn query_returns_one_open_row() {
        let items = links().query(&input("github.com/x"));
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id.0, "links:https://github.com/x");
        assert_eq!(it.title, "Open https://github.com/x");
        assert_eq!(it.subtitle, "Link");
        assert_eq!(it.source, "links");
        assert_eq!(it.actions, vec![ActionKind::Primary]);
        assert_eq!(
            it.payload,
            serde_json::json!({ "target": "https://github.com/x", "type": "url" })
        );

        let items = links().query(&input(r"~\Downloads"));
        assert_eq!(items[0].subtitle, "Path");
        assert_eq!(
            items[0].payload,
            serde_json::json!({ "target": r"C:\Users\me\Downloads", "type": "path" })
        );
        assert!(links().query(&input("hello world")).is_empty());
    }

    #[test]
    fn query_is_fast() {
        let l = links();
        let t = std::time::Instant::now();
        for _ in 0..100 {
            let _ = l.query(&input(r"%APPDATA%\Foo\bar"));
        }
        assert!(t.elapsed() < std::time::Duration::from_millis(5));
    }

    #[test]
    fn stop_drops_vars() {
        let l = links();
        l.stop();
        assert!(l.query(&input(r"~\Downloads")).is_empty());
    }

    fn item(target: &str, kind: &str) -> Item {
        Item {
            id: ItemId(format!("links:{target}")),
            kind: ItemKind::Command,
            title: String::new(),
            subtitle: String::new(),
            icon: None,
            score: 1.0,
            source: "links".into(),
            actions: vec![ActionKind::Primary],
            payload: serde_json::json!({ "target": target, "type": kind }),
        }
    }

    fn primary() -> Action {
        Action {
            kind: ActionKind::Primary,
            confirm_token: None,
        }
    }

    #[tokio::test]
    async fn execute_rejects_untrusted_payloads() {
        let l = links();
        for (target, kind) in [
            ("javascript:alert(1)", "url"),
            ("file:///c:/x", "url"),
            (r"\\server\share", "path"),
            (r"C:\a\..\b", "path"),
            // Valid target, wrong type.
            ("https://github.com/x", "path"),
            (r"C:\x", "url"),
        ] {
            let r = l.execute(&item(target, kind), primary()).await;
            assert!(r.is_err(), "{target}");
        }
        let no_target = Item {
            payload: serde_json::json!({}),
            ..item("", "url")
        };
        assert!(l.execute(&no_target, primary()).await.is_err());
    }

    #[tokio::test]
    async fn execute_only_primary_and_missing_path_errors() {
        let l = links();
        let loc = Action {
            kind: ActionKind::Location,
            confirm_token: None,
        };
        assert!(l.execute(&item(r"C:\x", "path"), loc).await.is_err());

        let dir = tempfile::tempdir().expect("temp dir");
        let p = dir.path().join("missing").to_string_lossy().into_owned();
        let r = l.execute(&item(&p, "path"), primary()).await;
        assert!(
            matches!(r, Err(FeatureError::Other(ref m)) if m == "That path does not exist"),
            "{r:?}"
        );
    }
}
