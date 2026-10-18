//! `config.toml` model (BSD section 6.1). No secret is ever stored here.

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
}

/// `[general]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Launcher hotkey.
    pub hotkey: String,
    /// Explain hotkey.
    pub explain_hotkey: String,
    /// `system`, `dark` or `light`.
    pub theme: String,
    /// Start with Windows.
    pub launch_at_startup: bool,
    /// Whether the first-run flow finished.
    pub first_run_done: bool,
}

/// `[window]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Window {
    /// Window width in pixels.
    pub width_px: u32,
    /// Distance from the top as a percent of screen height.
    pub top_percent: u32,
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
}

impl Default for General {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            explain_hotkey: "Alt+Shift+Space".into(),
            theme: "system".into(),
            launch_at_startup: false,
            first_run_done: false,
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width_px: 700,
            top_percent: 28,
            max_results: 5,
        }
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
        }
    }
}

impl Default for QueryMode {
    fn default() -> Self {
        Self {
            enabled: true,
            effort: "fast".into(),
            max_tokens: 1024,
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
            order: ["laya", "kev", "llm", "jev"].map(String::from).to_vec(),
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
            window: Window::default(),
            modes: Modes::default(),
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
        toml::from_str(&text).map_err(|source| CoreError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })
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

/// `%LOCALAPPDATA%\<APP_DISPLAY_NAME>`, parent of `cache\`, `data\` and `logs\` (BSD section 6.3).
pub fn local_data_dir() -> Result<PathBuf, CoreError> {
    env_dir("LOCALAPPDATA")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn round_trip_and_atomic_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("config.toml");
        let mut c = Config::default();
        c.general.theme = "dark".into();
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
        assert_eq!(c.general.theme, "light");
        assert_eq!(c.general.hotkey, "Alt+Space");
    }
}
