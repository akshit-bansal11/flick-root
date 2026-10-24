//! Theme engine: seed colours in, contrast-checked roles out.

pub mod color;
pub mod derive;
pub mod registry;

use serde::{Deserialize, Serialize};

pub use color::Rgb;
pub use derive::{Ratios, Resolved, Roles, resolve};
pub use registry::{Registry, Theme};

/// Errors returned by the theme engine. Messages never carry user text.
#[derive(thiserror::Error, Debug)]
pub enum ThemeError {
    /// A seed colour field is not `#rgb` or `#rrggbb`.
    #[error("bad colour in field {field}")]
    BadColour {
        /// Name of the seed field that failed to parse.
        field: &'static str,
    },
    /// A theme id is empty, malformed, or already taken.
    #[error("bad or duplicate theme id")]
    BadId,
    /// A theme seed list is not valid JSON for the seed schema.
    #[error("theme JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Where a seed comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Written for the app.
    Builtin,
    /// Ported from an upstream theme with a compatible licence.
    Port,
    /// Imported by the user.
    Imported,
}

/// A theme as authored: a handful of hex colours; the rest is derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seed {
    /// Kebab-case id (`imported:<slug>` for imported themes).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Origin of the seed.
    pub source: Source,
    /// Window background, `#rrggbb`.
    pub bg: String,
    /// Launcher panel colour.
    pub panel: String,
    /// Body text colour.
    pub text: String,
    /// Secondary text colour.
    pub muted: String,
    /// Accent colour.
    pub accent: String,
    /// Selected-row colour; derived when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sel: Option<String>,
    /// Border colour; derived when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// Raised-surface colour; derived when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// Upstream theme URL for ports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}
