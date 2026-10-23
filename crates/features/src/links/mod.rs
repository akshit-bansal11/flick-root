//! Links and paths: a typed http(s) URL or an absolute Windows path gets one Open row.
//!
//! `query` only parses text; it never touches the filesystem, the environment or the network.
//! The `%NAME%` and `~` expansions read a small allow-list of variables captured in `start`.
//! Opening happens only in `execute`, which re-validates the target and, for a path, resolves it
//! to its canonical form first. Folders and allow-listed document types open themselves; any
//! other file opens its parent folder, so a typed path never starts a program. Targets go to the
//! shell-open helper as a file name, never as a command line.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;

/// Longest text that is classified at all, and longest normalised target that is offered.
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

/// Document, media and archive types a typed path may open directly. Every other file opens its
/// parent folder instead, so programs, scripts, shortcuts and unknown types never run.
const SAFE_EXTS: &[&str] = &[
    "pdf", "txt", "md", "rtf", "csv", "log", "json", "xml", "yaml", "yml", "toml", "ini", "doc",
    "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "epub", "png", "jpg", "jpeg", "gif",
    "webp", "bmp", "svg", "ico", "tif", "tiff", "heic", "mp3", "wav", "flac", "ogg", "m4a", "aac",
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "zip", "7z", "rar", "tar", "gz",
];

/// Shown when a typed path cannot be resolved (missing, unreadable, or not on a drive).
const MISSING_MSG: &str = "That path does not exist";

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
        // Normalising can grow the text (percent-encoding); a target `execute` would refuse to
        // re-classify is never offered.
        .filter(|target| target.as_str().len() <= MAX_LEN)
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
        // `%APPDATA%foo` would glue the value and the name together; a separator must follow.
        if !tail.starts_with('\\') {
            return None;
        }
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
        || expanded
            .split(['\\', '/'])
            .any(|c| c == ".." || is_device_name(c))
    {
        return None;
    }
    Some(expanded)
}

/// True for a DOS device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`), with
/// or without an extension. Windows ignores the extension and trailing spaces on these names.
fn is_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    match stem.as_bytes() {
        b"CON" | b"PRN" | b"AUX" | b"NUL" => true,
        [b'C', b'O', b'M', d] | [b'L', b'P', b'T', d] => (b'1'..=b'9').contains(d),
        _ => false,
    }
}

/// True when the last name in `path` has an extension in [`SAFE_EXTS`]. Pure string check, no
/// I/O. Windows drops trailing dots and spaces from names, so `a.pdf.` counts as `a.pdf`.
fn is_safe_doc(path: &str) -> bool {
    let name = path
        .trim_end_matches(['.', ' ', '\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default();
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| SAFE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

/// Turns a path from `std::fs::canonicalize` into a plain drive path. `None` when it does not
/// resolve to a drive path (a network share or a device), which `execute` refuses.
fn plain_drive_path(canonical: &str) -> Option<&str> {
    let p = canonical.strip_prefix(r"\\?\").unwrap_or(canonical);
    is_drive_abs(p).then_some(p)
}

/// What Enter opens for an existing canonical path: a folder or a safe document opens itself,
/// any other file opens its parent folder. Pure, so the decision is testable without the shell.
fn open_target(canonical: &str, is_dir: bool) -> Option<&str> {
    if is_dir || is_safe_doc(canonical) {
        return Some(canonical);
    }
    Path::new(canonical).parent()?.to_str()
}

/// Resolves `p` (8.3 short names, symlinks and junctions included) and opens what
/// [`open_target`] decides. Blocking: call on the blocking pool with COM initialised.
fn open_typed_path(p: &str) -> Result<(), String> {
    let missing = || MISSING_MSG.to_owned();
    let canonical = std::fs::canonicalize(p).map_err(|_| missing())?;
    let canonical = canonical.to_str().ok_or_else(missing)?;
    let plain = plain_drive_path(canonical).ok_or_else(missing)?;
    let is_dir = std::fs::metadata(plain).map_err(|_| missing())?.is_dir();
    let target = open_target(plain, is_dir).ok_or_else(missing)?;
    index_apps::launch::open_path(Path::new(target))
        .map_err(|_| "Could not open that path".to_owned())
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
        // No I/O here: a name that does not look like a safe document is a folder, or a file whose
        // parent folder opens; either way a folder opens.
        let verb = match target {
            Target::Path(_) if !is_safe_doc(s) => "Open folder",
            _ => "Open",
        };
        vec![Item {
            id: ItemId(format!("links:{s}")),
            kind: ItemKind::Command,
            title: format!("{verb} {s}"),
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
        // User text (the URL or path) never goes into an error: errors reach the UI and the logs.
        let opened = tokio::task::spawn_blocking(move || {
            let _com = index_apps::com::ComGuard::new()
                .map_err(|_| "Could not start the shell".to_owned())?;
            match target {
                Target::Url(u) => index_apps::launch::open_path(Path::new(&u))
                    .map_err(|_| "Could not open that link".to_owned()),
                Target::Path(p) => open_typed_path(&p),
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
            r"%APPDATA%foo",
            r"%APPDATA%",
            &long,
        ];
        for text in cases {
            assert_eq!(c(text), None, "{text:?}");
        }
    }

    #[test]
    fn safe_documents_open_and_other_files_open_their_folder() {
        for p in [
            r"C:\x\notes.txt",
            r"C:\x\Report.PDF",
            r"C:\x\a.exe.txt",
            r"C:\x\a.tar.gz",
            r"C:\x\a.pdf.",
        ] {
            assert_eq!(open_target(p, false), Some(p), "{p}");
            assert!(
                links().query(&input(p))[0].title.starts_with("Open C:"),
                "{p}"
            );
        }
        for p in [
            r"C:\x\a.py",
            r"C:\x\a.exe",
            r"C:\x\A.EXE ",
            r"C:\x\a.appref-ms",
            r"C:\x\setup.msi",
            r"C:\x\noext",
            r"C:\x\a.pdf.exe",
        ] {
            assert_eq!(open_target(p, false), Some(r"C:\x"), "{p}");
            assert!(
                links().query(&input(p))[0]
                    .title
                    .starts_with("Open folder "),
                "{p}"
            );
        }
        // A folder opens itself whatever its name looks like.
        assert_eq!(open_target(r"C:\x\tool.exe", true), Some(r"C:\x\tool.exe"));
        // Links are never affected by the extension check.
        assert_eq!(
            links().query(&input("github.com/a.exe"))[0].title,
            "Open https://github.com/a.exe"
        );
    }

    #[test]
    fn decision_uses_the_canonical_name() {
        // `A~1.PDF` is the 8.3 short name of `a.pdfx`, and a link named `doc.pdf` can point at a
        // program. The row reads from the typed text; Enter decides on the resolved name.
        assert!(is_safe_doc(r"C:\x\A~1.PDF"));
        assert_eq!(open_target(r"C:\x\a.pdfx", false), Some(r"C:\x"));
        assert_eq!(open_target(r"C:\y\evil.exe", false), Some(r"C:\y"));
        assert_eq!(plain_drive_path(r"\\?\C:\x\a.pdfx"), Some(r"C:\x\a.pdfx"));
        assert_eq!(plain_drive_path(r"C:\x"), Some(r"C:\x"));
        assert_eq!(plain_drive_path(r"\\?\UNC\server\share\a.pdf"), None);
        assert_eq!(plain_drive_path(r"\\.\CON"), None);
    }

    #[test]
    fn rejects_device_names() {
        for p in [
            r"C:\CON",
            r"C:\x\con.txt",
            r"C:\x\NUL",
            r"C:\x\aux.tar.gz",
            r"C:\x\Prn \y",
            r"C:\x\COM1",
            r"C:\x\lpt9.log",
            "C:/x/com3/y",
        ] {
            assert_eq!(c(p), None, "{p}");
        }
        for p in [
            r"C:\x\COM0",
            r"C:\x\console",
            r"C:\x\LPT10",
            r"C:\x\nullable.txt",
        ] {
            assert!(c(p).is_some(), "{p}");
        }
    }

    #[test]
    fn drops_a_url_that_grows_past_the_limit() {
        // 600 two-byte characters fit the input limit but grow to 3,600 bytes once encoded.
        let text = format!("https://example.com/{}", "é".repeat(600));
        assert!(text.len() <= MAX_LEN);
        assert_eq!(c(&text), None);
        assert!(links().query(&input(&text)).is_empty());
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
