//! Shared types that cross the Rust and UI boundary (BSD section 2).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Launcher mode, chosen by prefix or Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Apps and commands.
    Command,
    /// File search.
    Find,
    /// Ask a model.
    Query,
}

/// One keystroke's worth of input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    /// Text as typed, prefix included.
    pub raw: String,
    /// Text with the mode prefix removed.
    pub text: String,
    /// Active mode.
    pub mode: Mode,
    /// Monotonic sequence number, used to drop stale results.
    pub seq: u64,
}

/// Stable identifier of a result item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);

/// What kind of thing an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// Installed application.
    App,
    /// Built-in command.
    Command,
    /// File.
    File,
    /// Folder.
    Folder,
    /// Project folder.
    Project,
    /// Computed result.
    Result,
    /// Running process.
    Process,
    /// Media control.
    Media,
    /// Capture target.
    Capture,
    /// Disambiguation choice.
    Choice,
}

/// Reference to an icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum IconRef {
    /// Icon file on disk.
    File(PathBuf),
    /// Inline data URL.
    DataUrl(String),
    /// Named glyph.
    Glyph(String),
}

/// An action that can be run on an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ActionKind {
    /// Default action (Enter).
    Primary,
    /// Run as administrator.
    Admin,
    /// Open containing folder.
    Location,
    /// Copy the path.
    CopyPath,
    /// Copy the result text.
    CopyResult,
    /// Kill a process.
    Kill,
    /// Feature-defined action.
    Custom(String),
}

/// A result row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Stable id.
    pub id: ItemId,
    /// Kind of item.
    pub kind: ItemKind,
    /// Main line.
    pub title: String,
    /// Secondary line.
    pub subtitle: String,
    /// Optional icon.
    pub icon: Option<IconRef>,
    /// Ranking score, higher first.
    pub score: f32,
    /// Feature id or "apps"/"files".
    pub source: String,
    /// Actions offered, first is primary.
    pub actions: Vec<ActionKind>,
    /// Feature-specific data.
    pub payload: serde_json::Value,
}

/// A request to run an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// Which action.
    pub kind: ActionKind,
    /// Token from an earlier `NeedsConfirm`, when confirming.
    pub confirm_token: Option<String>,
}

/// Result of executing an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Outcome {
    /// Finished, launcher stays.
    Done,
    /// Finished, hide the launcher.
    HideLauncher,
    /// Text copied to the clipboard.
    Copied,
    /// Needs a second confirm within the window.
    #[serde(rename_all = "camelCase")]
    NeedsConfirm {
        /// Confirmation token.
        token: String,
        /// Milliseconds until the token expires.
        expires_ms: u64,
    },
    /// Replace the list with these items.
    ShowItems {
        /// The items.
        items: Vec<Item>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_is_tagged_camel_case() {
        let o = Outcome::NeedsConfirm {
            token: "t".into(),
            expires_ms: 5,
        };
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["type"], "needsConfirm");
        assert_eq!(v["expiresMs"], 5);
    }

    #[test]
    fn action_round_trips() {
        let a = Action {
            kind: ActionKind::Custom("x".into()),
            confirm_token: None,
        };
        let s = serde_json::to_string(&a).unwrap();
        assert!(s.contains("confirmToken"));
        assert_eq!(serde_json::from_str::<Action>(&s).unwrap(), a);
    }
}
