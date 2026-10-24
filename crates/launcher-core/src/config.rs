//! `config.toml` model. No secret is ever stored here.

use crate::{brand, error::CoreError};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Current config schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Whole config file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Schema version of this file.
    pub schema_version: u32,
    /// General settings.
    pub general: General,
    /// Colour scheme and the two theme slots.
    pub appearance: Appearance,
    /// Launcher window geometry.
    pub window: Window,
    /// Per-mode settings.
    pub modes: Modes,
    /// Model providers, all disabled by default.
    pub providers: Providers,
    /// Decision engines, all disabled by default.
    pub decision: Decision,
    /// Feature options keyed by feature id; each table has `enabled` plus options.
    pub features: BTreeMap<String, toml::Table>,
    /// Third-party integrations (no secrets).
    pub integrations: Integrations,
    /// Installed-app and program search.
    pub apps: AppsConfig,
}

/// `[apps]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppsConfig {
    /// Scan every fixed drive for exe, msi, apk and other launchable files.
    pub scan_programs: bool,
    /// Folders excluded from the programs scan (see [`validate_skip`] for the entry forms).
    /// Absent in an old config means [`DEFAULT_SKIP`]; `skip = []` means nothing is excluded.
    pub skip: Vec<String>,
}

impl Default for AppsConfig {
    fn default() -> Self {
        Self {
            scan_programs: true,
            skip: DEFAULT_SKIP.map(String::from).to_vec(),
        }
    }
}

/// Exclusions a fresh config starts with, each removable in Settings. Reasons: `winsxs` holds
/// tens of thousands of hard-linked duplicates, `node_modules` per-project shims, `recent` a
/// shortcut per opened document, `site-packages` pip launcher stubs, and the Cargo `target`
/// intermediates hashed test binaries (`target\release\app.exe` stays indexed).
pub const DEFAULT_SKIP: [&str; 7] = [
    "winsxs",
    "node_modules",
    "recent",
    "site-packages",
    r"target\*\deps",
    r"target\*\build",
    r"target\*\incremental",
];
/// Most entries `[apps] skip` may hold.
pub const MAX_SKIP: usize = 200;
/// Longest single skip entry, in characters.
pub const MAX_SKIP_LEN: usize = 260;

/// Why a skip entry or list was rejected. Never carries the entry text.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipError {
    /// The entry is empty after trimming.
    #[error("skip entry is empty")]
    Empty,
    /// The entry is longer than [`MAX_SKIP_LEN`].
    #[error("skip entry is too long")]
    TooLong,
    /// The entry holds a control character.
    #[error("skip entry has a control character")]
    Control,
    /// The entry has a `..` segment.
    #[error("skip entry has a .. segment")]
    ParentDir,
    /// UNC, device (`\\?\`, `\\.\`) or drive-less rooted path.
    #[error("skip entry is a UNC, device or rooted path")]
    Unc,
    /// A character Windows paths cannot hold, `:` outside the drive, or a wildcard other than `*`.
    #[error("skip entry has a character that is not allowed")]
    BadChar,
    /// An empty or `.` segment.
    #[error("skip entry has an empty segment")]
    Malformed,
    /// The pattern is only `*`, which would exclude everything.
    #[error("skip entry is only wildcards")]
    OnlyWildcards,
    /// A `*` in an entry that has a drive; only drive-less patterns may use wildcards.
    #[error("wildcards are only allowed in patterns without a drive")]
    WildcardWithDrive,
    /// More than [`MAX_SKIP`] entries.
    #[error("too many skip entries")]
    TooMany,
}

/// Validate and normalise one entry: trim, `/` to `\`, `e:` to `E:\`. Accepts a drive (`E:\`), a
/// path under a drive (`E:\games`), or a pattern without a drive (`node_modules`, `*cache*`,
/// `target\*\deps`) that matches any run of consecutive folder names. Entries are only ever
/// compared to paths, never executed or opened.
fn validate_entry(raw: &str) -> Result<String, SkipError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(SkipError::Empty);
    }
    if t.chars().count() > MAX_SKIP_LEN {
        return Err(SkipError::TooLong);
    }
    if t.chars().any(char::is_control) {
        return Err(SkipError::Control);
    }
    let s = t.replace('/', "\\");
    if s.starts_with('\\') {
        return Err(SkipError::Unc);
    }
    let b = s.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    if drive && b.len() > 2 && b[2] != b'\\' {
        return Err(SkipError::BadChar);
    }
    let body = if drive { &s[2..] } else { &s[..] };
    if body.contains([':', '<', '>', '"', '|', '?']) {
        return Err(SkipError::BadChar);
    }
    let trimmed = body.trim_matches('\\');
    let mut segs = Vec::new();
    if !trimmed.is_empty() {
        for seg in trimmed.split('\\') {
            match seg {
                "" | "." => return Err(SkipError::Malformed),
                ".." => return Err(SkipError::ParentDir),
                _ => segs.push(seg),
            }
        }
    }
    if drive && segs.iter().any(|g| g.contains('*')) {
        return Err(SkipError::WildcardWithDrive);
    }
    if drive {
        let letter = b[0].to_ascii_uppercase() as char;
        return Ok(format!("{letter}:\\{}", segs.join("\\")));
    }
    if segs.iter().all(|g| g.bytes().all(|c| c == b'*')) {
        return Err(SkipError::OnlyWildcards);
    }
    Ok(segs.join("\\"))
}

/// Validate a skip list from the UI: every entry through the entry rules (empty, over
/// [`MAX_SKIP_LEN`], control characters, `..`, UNC or device prefixes and `?` are rejected),
/// then deduplicated case-insensitively. More than [`MAX_SKIP`] entries is an error.
pub fn validate_skip(entries: Vec<String>) -> Result<Vec<String>, SkipError> {
    if entries.len() > MAX_SKIP {
        return Err(SkipError::TooMany);
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(entries.len());
    for raw in &entries {
        let e = validate_entry(raw)?;
        if seen.insert(e.to_lowercase()) {
            out.push(e);
        }
    }
    Ok(out)
}

/// Load-time cleanup of a hand-edited list: invalid entries are dropped (never a crash),
/// duplicates removed, the list cut at [`MAX_SKIP`]. Only a hand edit of config.toml can add an
/// invalid entry (the settings command validates); Settings shows the list in effect.
fn sanitize_skip(entries: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in entries.iter().filter_map(|raw| validate_entry(raw).ok()) {
        if out.len() < MAX_SKIP && seen.insert(e.to_lowercase()) {
            out.push(e);
        }
    }
    out
}

/// `[general]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Launcher hotkey.
    pub hotkey: String,
    /// Explain hotkey.
    pub explain_hotkey: String,
    /// Pre-appearance `theme` key; read once and moved to `appearance.scheme` on load, never written.
    #[serde(rename = "theme", default, skip_serializing)]
    pub legacy_theme: Option<String>,
    /// Start with Windows.
    pub launch_at_startup: bool,
    /// Whether the first-run flow finished.
    pub first_run_done: bool,
    /// Whether the tutorial was finished or skipped.
    pub tutorial_done: bool,
    /// Ask GitHub at most once a day whether a newer release exists.
    pub check_updates: bool,
}

/// Colour schemes.
pub const SCHEMES: [&str; 3] = ["system", "dark", "light"];

/// `[appearance]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Pre-themes `theme` key; read once by [`Appearance::migrate_legacy`], never written.
    #[serde(rename = "theme", default, skip_serializing)]
    pub legacy_theme: Option<String>,
    /// One of [`SCHEMES`].
    pub scheme: String,
    /// Theme id used in the dark scheme; absent means the built-in dark default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_theme: Option<String>,
    /// Theme id used in the light scheme; absent means the built-in light default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_theme: Option<String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            legacy_theme: None,
            scheme: "system".into(),
            dark_theme: None,
            light_theme: None,
        }
    }
}

/// Keeps only `[a-z0-9:-]` so an id can be shown in an error without echoing arbitrary text.
fn safe_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '-'))
        .collect()
}

impl Appearance {
    /// Theme id for the dark scheme: the stored id or [`brand::DEFAULT_THEME_ID`].
    pub fn dark_id(&self) -> &str {
        self.dark_theme
            .as_deref()
            .unwrap_or(brand::DEFAULT_THEME_ID)
    }

    /// Theme id for the light scheme: the stored id or [`brand::DEFAULT_THEME_LIGHT_ID`].
    pub fn light_id(&self) -> &str {
        self.light_theme
            .as_deref()
            .unwrap_or(brand::DEFAULT_THEME_LIGHT_ID)
    }

    /// Maps the legacy `theme` value onto the two theme slots, once: the value is consumed, and
    /// nothing changes while a slot is already set. `frost` becomes the light theme, `midnight`,
    /// `aurora` and `ember` the dark theme; any other value (`glass`, `solid`) leaves the defaults.
    /// When the old theme forced a scheme and `scheme` is `system`, the scheme becomes that one so
    /// the look is kept.
    pub fn migrate_legacy(&mut self) {
        let Some(theme) = self.legacy_theme.take() else {
            return;
        };
        if self.dark_theme.is_some() || self.light_theme.is_some() {
            return;
        }
        let dark = match theme.as_str() {
            "frost" => false,
            "midnight" | "aurora" | "ember" => true,
            _ => return,
        };
        if dark {
            self.dark_theme = Some(theme);
        } else {
            self.light_theme = Some(theme);
        }
        if self.scheme == "system" {
            self.scheme = if dark { "dark" } else { "light" }.into();
        }
    }

    /// Checks the scheme and both theme slots against `lookup`, which returns `Some(is_dark)` for a
    /// known id and `None` for an unknown one. The dark slot must hold a dark theme, the light slot
    /// a light one.
    pub fn validate_themes(&self, lookup: &dyn Fn(&str) -> Option<bool>) -> Result<(), String> {
        if !SCHEMES.contains(&self.scheme.as_str()) {
            return Err("scheme must be system, dark or light".into());
        }
        for (slot, id, want_dark) in [
            ("dark_theme", self.dark_id(), true),
            ("light_theme", self.light_id(), false),
        ] {
            match lookup(id) {
                None => return Err(format!("{slot}: unknown theme {}", safe_id(id))),
                Some(is_dark) if is_dark != want_dark => {
                    let kind = if want_dark { "dark" } else { "light" };
                    return Err(format!("{slot}: {} is not a {kind} theme", safe_id(id)));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}

/// `[window]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Window {
    /// Maximum visible results.
    pub max_results: u32,
}

/// `[modes]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Modes {
    /// Find mode.
    pub find: FindMode,
    /// Query mode.
    pub query: QueryMode,
}

/// `[modes.find]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FindMode {
    /// Find mode enabled.
    pub enabled: bool,
    /// Indexed roots (known folder names or paths).
    pub roots: Vec<String>,
    /// User-added roots.
    pub extra_roots: Vec<String>,
    /// Also index every fixed, non-system drive root (D:\, E:\, ...).
    pub include_drives: bool,
}

/// `[modes.query]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueryMode {
    /// Query mode enabled.
    pub enabled: bool,
    /// `fast` or `think`.
    pub effort: String,
    /// Output token cap.
    pub max_tokens: u32,
    /// Last chosen thinking mode id (`<provider>/<model>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_mode: Option<String>,
}

/// One model provider entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderEntry {
    /// Enabled by the user.
    pub enabled: bool,
    /// Wire format: `openai` or `anthropic`.
    pub wire: String,
    /// Base URL.
    pub base_url: String,
    /// Model name.
    pub model: String,
    /// Extra models besides `model`, offered as thinking modes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    /// Host the key was entered for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_host: Option<String>,
    /// Optional per-provider effort override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

/// `[providers]`: an `order` list plus one table per provider id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Providers {
    /// Fallback order of provider ids.
    pub order: Vec<String>,
    /// Provider tables by id.
    #[serde(flatten)]
    pub entries: BTreeMap<String, ProviderEntry>,
}

/// One decision engine entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EngineEntry {
    /// Enabled by the user.
    pub enabled: bool,
    /// Base URL, when the engine is a service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Host the key was entered for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_host: Option<String>,
    /// Request timeout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// `[decision]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    /// Engine order; the built-in parser always runs first.
    pub order: Vec<String>,
    /// Maximum candidates sent.
    pub candidates: u32,
    /// Minimum probability to accept an answer.
    pub threshold: f64,
    /// Engine tables by id.
    #[serde(flatten)]
    pub engines: BTreeMap<String, EngineEntry>,
}

/// `[integrations]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Integrations {
    /// Spotify.
    pub spotify: Spotify,
    /// YouTube.
    pub youtube: Youtube,
    /// Obsidian.
    pub obsidian: Obsidian,
    /// Notion.
    pub notion: Notion,
    /// Webhooks; the URL itself lives in Credential Manager.
    pub webhooks: Vec<Webhook>,
    /// Markdown capture file (empty when unset).
    pub markdown_file: String,
}

/// `[integrations.spotify]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spotify {
    /// Public PKCE client id.
    pub client_id: String,
    /// Loopback redirect port.
    pub redirect_port: u16,
}

/// `[integrations.youtube]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Youtube {
    /// Use the YouTube API.
    pub use_api: bool,
}

/// `[integrations.obsidian]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Obsidian {
    /// Vault path.
    pub vault: String,
    /// Daily note file name format.
    pub daily_note_format: String,
    /// Daily note folder.
    pub daily_folder: String,
}

/// `[integrations.notion]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Notion {
    /// Database id.
    pub database_id: String,
}

/// `[[integrations.webhooks]]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Webhook {
    /// Display name.
    pub name: String,
    /// Enabled flag.
    pub enabled: bool,
    /// Host of the secret URL; the secret is bound to it (not secret itself).
    pub host: String,
}

impl Default for General {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            explain_hotkey: "Alt+Shift+Space".into(),
            legacy_theme: None,
            launch_at_startup: false,
            first_run_done: false,
            tutorial_done: false,
            check_updates: true,
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self { max_results: 5 }
    }
}

impl Default for FindMode {
    fn default() -> Self {
        Self {
            enabled: true,
            roots: [
                "Desktop",
                "Documents",
                "Downloads",
                "Pictures",
                "Videos",
                "Music",
            ]
            .map(String::from)
            .to_vec(),
            extra_roots: Vec::new(),
            include_drives: true,
        }
    }
}

impl Default for QueryMode {
    fn default() -> Self {
        Self {
            enabled: true,
            effort: "fast".into(),
            max_tokens: 1024,
            thinking_mode: None,
        }
    }
}

impl Default for ProviderEntry {
    fn default() -> Self {
        provider("openai", "", None, "")
    }
}

fn provider(wire: &str, base_url: &str, key_host: Option<&str>, model: &str) -> ProviderEntry {
    ProviderEntry {
        enabled: false,
        wire: wire.into(),
        base_url: base_url.into(),
        model: model.into(),
        models: Vec::new(),
        key_host: key_host.map(String::from),
        effort: None,
    }
}

impl Default for Providers {
    fn default() -> Self {
        let table = [
            (
                "ollama",
                provider("openai", "http://localhost:11434/v1", None, ""),
            ),
            (
                "lmstudio",
                provider("openai", "http://localhost:1234/v1", None, ""),
            ),
            (
                "llamacpp",
                provider("openai", "http://localhost:8080/v1", None, ""),
            ),
            (
                "omniroute",
                provider("openai", "http://localhost:20128/v1", None, "auto"),
            ),
            (
                "openrouter",
                provider(
                    "openai",
                    "https://openrouter.ai/api/v1",
                    Some("openrouter.ai"),
                    "",
                ),
            ),
            (
                "anthropic",
                provider(
                    "anthropic",
                    "https://api.anthropic.com",
                    Some("api.anthropic.com"),
                    "claude-haiku-5-5",
                ),
            ),
            (
                "openai",
                provider(
                    "openai",
                    "https://api.openai.com/v1",
                    Some("api.openai.com"),
                    "",
                ),
            ),
            ("custom", provider("openai", "", Some(""), "")),
        ];
        Self {
            order: table.iter().map(|(id, _)| (*id).to_string()).collect(),
            entries: table
                .into_iter()
                .map(|(id, e)| (id.to_string(), e))
                .collect(),
        }
    }
}

impl Default for Decision {
    fn default() -> Self {
        let engine = |url: Option<&str>, host: Option<&str>, timeout: Option<u64>| EngineEntry {
            enabled: false,
            base_url: url.map(String::from),
            key_host: host.map(String::from),
            timeout_ms: timeout,
        };
        let engines = BTreeMap::from([
            (
                "laya".to_string(),
                engine(Some("http://localhost:8000"), None, None),
            ),
            ("kev".to_string(), engine(Some(""), None, None)),
            ("custom".to_string(), engine(Some(""), None, None)),
            ("llm".to_string(), engine(None, None, None)),
            (
                "jev".to_string(),
                engine(
                    Some("https://api.typesafe.ai"),
                    Some("api.typesafe.ai"),
                    Some(4000),
                ),
            ),
        ]);
        Self {
            order: ["laya", "kev", "llm", "jev", "custom"]
                .map(String::from)
                .to_vec(),
            candidates: 8,
            threshold: 0.6,
            engines,
        }
    }
}

impl Default for Spotify {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            redirect_port: 43919,
        }
    }
}

impl Default for Obsidian {
    fn default() -> Self {
        Self {
            vault: String::new(),
            daily_note_format: "YYYY-MM-DD".into(),
            daily_folder: String::new(),
        }
    }
}

fn opts(pairs: Vec<(&str, toml::Value)>) -> toml::Table {
    let mut t = toml::Table::new();
    t.insert("enabled".into(), true.into());
    for (k, v) in pairs {
        t.insert(k.into(), v);
    }
    t
}

fn shortcut(keyword: &str, name: &str, url: &str) -> toml::Value {
    let mut t = toml::Table::new();
    t.insert("keyword".into(), keyword.into());
    t.insert("name".into(), name.into());
    t.insert("url".into(), url.into());
    toml::Value::Table(t)
}

fn default_features() -> BTreeMap<String, toml::Table> {
    let strings = |v: &[&str]| toml::Value::Array(v.iter().map(|s| (*s).into()).collect());
    let shortcuts = vec![
        shortcut("g", "Google", "https://www.google.com/search?q={q}"),
        shortcut("amz", "Amazon.in", "https://www.amazon.in/s?k={q}"),
        shortcut(
            "yt",
            "YouTube",
            "https://www.youtube.com/results?search_query={q}",
        ),
        shortcut("gh", "GitHub", "https://github.com/search?q={q}"),
        shortcut("maps", "Maps", "https://www.google.com/maps/search/{q}"),
        shortcut(
            "wiki",
            "Wikipedia",
            "https://en.wikipedia.org/w/index.php?search={q}",
        ),
    ];
    let projects = opts(vec![
        (
            "roots",
            strings(&["D:\\dev", "%USERPROFILE%\\source\\repos"]),
        ),
        (
            "markers",
            strings(&[
                ".git",
                "package.json",
                "Cargo.toml",
                "pyproject.toml",
                "go.mod",
            ]),
        ),
        ("editor", "vscode".into()),
        ("include_vscode_recents", true.into()),
    ]);
    BTreeMap::from([
        (
            "frecency".to_string(),
            opts(vec![
                ("half_life_days", 7.into()),
                ("learn_prefixes", true.into()),
            ]),
        ),
        ("math".into(), opts(vec![("currency_refresh", true.into())])),
        (
            "syscmd".into(),
            opts(vec![("confirm_window_ms", 3000.into())]),
        ),
        (
            "web".into(),
            opts(vec![("shortcuts", toml::Value::Array(shortcuts))]),
        ),
        ("projects".into(), projects),
        ("nlfind".into(), opts(vec![])),
        ("ports".into(), opts(vec![])),
        ("media".into(), opts(vec![])),
        ("explain".into(), opts(vec![("max_chars", 8000.into())])),
        ("capture".into(), opts(vec![("default_target", "".into())])),
    ])
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: General::default(),
            appearance: Appearance::default(),
            window: Window::default(),
            modes: Modes::default(),
            apps: AppsConfig::default(),
            providers: Providers::default(),
            decision: Decision::default(),
            features: default_features(),
            integrations: Integrations::default(),
        }
    }
}

impl Config {
    /// Load the config at `path`. A missing file yields the default config.
    pub fn load(path: &Path) -> Result<Config, CoreError> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(source) => {
                return Err(CoreError::ConfigIo {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let mut c: Config = toml::from_str(&text).map_err(|source| CoreError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })?;
        c.apps.skip = sanitize_skip(std::mem::take(&mut c.apps.skip));
        // Configs written before the Custom engine existed lack it in the order.
        if !c.decision.order.iter().any(|i| i == "custom") {
            c.decision.order.push("custom".into());
        }
        // Old configs kept the colour scheme in `[general] theme`.
        if let Some(t) = c.general.legacy_theme.take()
            && SCHEMES.contains(&t.as_str())
            && c.appearance.scheme == "system"
        {
            c.appearance.scheme = t;
        }
        c.appearance.migrate_legacy();
        Ok(c)
    }

    /// Save atomically: write `<path>.tmp`, then rename over `path`.
    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        let io = |source| CoreError::ConfigIo {
            path: path.to_path_buf(),
            source,
        };
        let text = toml::to_string_pretty(self)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        fs::write(&tmp, text).map_err(io)?;
        fs::rename(&tmp, path).map_err(io)
    }
}

fn env_dir(var: &'static str) -> Result<PathBuf, CoreError> {
    std::env::var_os(var)
        .map(|v| PathBuf::from(v).join(brand::APP_DISPLAY_NAME))
        .ok_or(CoreError::MissingEnv(var))
}

/// `%APPDATA%\<APP_DISPLAY_NAME>\config.toml`.
pub fn config_path() -> Result<PathBuf, CoreError> {
    env_dir("APPDATA").map(|d| d.join("config.toml"))
}

/// `%LOCALAPPDATA%\<APP_DISPLAY_NAME>`, parent of `cache\`, `data\` and `logs\`.
pub fn local_data_dir() -> Result<PathBuf, CoreError> {
    env_dir("LOCALAPPDATA")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_done_defaults_false_for_old_configs() {
        let c: Config = toml::from_str(
            "[general]
first_run_done = true
",
        )
        .unwrap();
        assert!(c.general.first_run_done);
        assert!(!c.general.tutorial_done);
    }

    #[test]
    fn thinking_fields_round_trip_and_old_configs_load() {
        let old: Config = toml::from_str(&toml::to_string(&Config::default()).unwrap()).unwrap();
        assert!(old.providers.entries["ollama"].models.is_empty());
        assert_eq!(old.modes.query.thinking_mode, None);
        let mut c = Config::default();
        c.modes.query.thinking_mode = Some("ollama/x".into());
        c.providers.entries.get_mut("ollama").unwrap().models = vec!["y".into()];
        let back: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        assert!(
            !toml::to_string(&Config::default())
                .unwrap()
                .contains("models")
        );
    }

    #[test]
    fn old_config_without_custom_engine_still_loads() {
        let dir = std::env::temp_dir().join(format!("flick-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("config.toml");
        std::fs::write(
            &f,
            "[decision]
order = [\"laya\", \"kev\", \"llm\", \"jev\"]
candidates = 5
threshold = 0.7
",
        )
        .unwrap();
        let c = Config::load(&f).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(c.decision.order.last().map(String::as_str), Some("custom"));
        assert_eq!(c.decision.candidates, 5);
        let d = Config::default().decision;
        assert_eq!(d.order.last().map(String::as_str), Some("custom"));
        assert!(!d.engines["custom"].enabled);
    }

    #[test]
    fn default_has_nothing_enabled_and_no_secrets() {
        let c = Config::default();
        assert!(c.providers.entries.values().all(|p| !p.enabled));
        assert!(c.decision.engines.values().all(|e| !e.enabled));
        let s = toml::to_string_pretty(&c).unwrap();
        // Line-anchored: "hotkey =" legitimately ends in "key =".
        for line in s.lines() {
            for bad in ["key =", "token =", "secret ="] {
                assert!(!line.trim_start().starts_with(bad), "{line}");
            }
        }
    }

    #[test]
    fn old_integrations_without_new_fields_load() {
        let c: Config = toml::from_str(
            "[integrations]\n[[integrations.webhooks]]\nname = \"a\"\nenabled = true\n",
        )
        .unwrap();
        assert_eq!(c.integrations.webhooks[0].host, "");
        assert_eq!(c.integrations.markdown_file, "");
    }

    #[test]
    fn round_trip_and_atomic_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("config.toml");
        let mut c = Config::default();
        c.appearance.scheme = "dark".into();
        c.save(&p).unwrap();
        assert!(!p.with_extension("toml.tmp").exists());
        assert_eq!(Config::load(&p).unwrap(), c);
    }

    #[test]
    fn missing_file_is_default_and_bad_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        assert_eq!(Config::load(&p).unwrap(), Config::default());
        fs::write(&p, "schema_version = [").unwrap();
        assert!(matches!(
            Config::load(&p),
            Err(CoreError::ConfigParse { .. })
        ));
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        fs::write(&p, "[general]\ntheme = \"light\"\n").unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.appearance.scheme, "light");
        assert_eq!(c.appearance.dark_theme, None);
        assert_eq!(c.appearance.light_theme, None);
        assert_eq!(c.general.hotkey, "Alt+Space");
    }

    #[test]
    fn old_appearance_keys_load_and_are_not_written_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        fs::write(&p, "[appearance]\ntheme = \"midnight\"\nglass = 62\n").unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.appearance.dark_theme.as_deref(), Some("midnight"));
        assert_eq!(c.appearance.scheme, "dark");
        assert_eq!(c.appearance.legacy_theme, None);
        c.save(&p).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(!text.contains("glass"));
    }
}

#[cfg(test)]
mod find_drive_tests {
    use super::*;

    #[test]
    fn old_config_defaults_include_drives() {
        let old = "[modes.find]\nenabled = true\nroots = [\"Desktop\"]\n";
        let c: Config = toml::from_str(old).unwrap();
        assert!(c.modes.find.include_drives);
        let off: Config = toml::from_str("[modes.find]\ninclude_drives = false\n").unwrap();
        assert!(!off.modes.find.include_drives);
    }

    #[test]
    fn old_config_defaults_scan_programs() {
        let c: Config = toml::from_str(
            "[general]
theme = \"light\"
",
        )
        .unwrap();
        assert!(c.apps.scan_programs);
        let off: Config = toml::from_str(
            "[apps]
scan_programs = false
",
        )
        .unwrap();
        assert!(!off.apps.scan_programs);
    }

    #[test]
    fn old_config_gets_default_skip_and_empty_stays_empty() {
        let c: Config = toml::from_str("[apps]\nscan_programs = true\n").unwrap();
        assert_eq!(c.apps.skip, DEFAULT_SKIP.map(String::from).to_vec());
        let e: Config = toml::from_str("[apps]\nskip = []\n").unwrap();
        assert!(e.apps.skip.is_empty());
    }

    #[test]
    fn load_drops_invalid_skip_entries() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        fs::write(
            &p,
            "[apps]\nskip = [\"Good\", \"..\\\\x\", \"\", \"\\\\\\\\srv\\\\s\", \"good\", \"e:\"]\n",
        )
        .unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.apps.skip, ["Good", r"E:\"]);
    }

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn validate_skip_rejects() {
        let long = "a".repeat(MAX_SKIP_LEN + 1);
        let table: [(&str, SkipError); 17] = [
            ("", SkipError::Empty),
            ("   ", SkipError::Empty),
            (long.as_str(), SkipError::TooLong),
            ("a\u{7}b", SkipError::Control),
            ("a\nb", SkipError::Control),
            (r"..\x", SkipError::ParentDir),
            (r"C:\a\..\b", SkipError::ParentDir),
            (r"\\?\C:\x", SkipError::Unc),
            (r"\\.\pipe", SkipError::Unc),
            (r"\\server\share", SkipError::Unc),
            (r"\rooted", SkipError::Unc),
            ("a?b", SkipError::BadChar),
            ("C:foo", SkipError::BadChar),
            ("*", SkipError::OnlyWildcards),
            (r"a\\b", SkipError::Malformed),
            (r"C:\*", SkipError::WildcardWithDrive),
            (r"D:\x*", SkipError::WildcardWithDrive),
        ];
        for (input, want) in table {
            assert_eq!(validate_skip(v(&[input])), Err(want), "{input:?}");
        }
        assert_eq!(validate_skip(v(&["**"])), Err(SkipError::OnlyWildcards));
        assert_eq!(validate_skip(v(&["a:b"])), Err(SkipError::BadChar));
        assert!(validate_skip(v(&["node_modules", r"target\*\deps"])).is_ok());
    }

    #[test]
    fn validate_skip_normalises_dedupes_and_caps() {
        let got = validate_skip(v(&[
            " e: ",
            "d:/Games/",
            "Node_Modules",
            "node_modules",
            "target/*/deps",
            ".*",
            "*cache*",
        ]))
        .unwrap();
        assert_eq!(
            got,
            [
                r"E:\",
                r"D:\Games",
                "Node_Modules",
                r"target\*\deps",
                ".*",
                "*cache*"
            ]
        );
        let ok: Vec<String> = (0..MAX_SKIP).map(|i| format!("f{i}")).collect();
        assert_eq!(validate_skip(ok.clone()).unwrap().len(), MAX_SKIP);
        let mut over = ok;
        over.push("extra".into());
        assert_eq!(validate_skip(over), Err(SkipError::TooMany));
    }

    #[test]
    fn default_skip_entries_are_valid() {
        let d = DEFAULT_SKIP.map(String::from).to_vec();
        assert_eq!(validate_skip(d.clone()).unwrap(), d);
    }

    #[test]
    fn old_window_keys_are_ignored() {
        let old = "[window]
width_px = 700
top_percent = 28
max_results = 7
";
        let c: Config = toml::from_str(old).unwrap();
        assert_eq!(c.window.max_results, 7);
    }
}
