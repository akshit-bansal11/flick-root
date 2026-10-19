//! F09 Explain this: a second hotkey captures the selected text and opens Query mode with it
//! attached (KICKOFF 1.3). The feature holds no query rows; it only gates the hotkey. The shell
//! binds the hotkey while [`is_running`] is true. The captured text is analysed here so it can be
//! shown before anything is sent, and secret-looking text is blocked.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::detect::looks_secret;
use launcher_core::{Action, Input, Item, Mode, Outcome};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

/// Characters of the captured text shown in the preview (and sent, if the user presses Enter).
pub const PREVIEW_CHARS: usize = 2000;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// True between `start` and `stop`; the shell binds the Explain hotkey only then.
pub fn is_running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

/// Payload of the `explain://captured` event (BSD 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Captured {
    /// The text the user will see, at most [`PREVIEW_CHARS`] characters; empty when blocked.
    pub preview: String,
    /// Length of the whole selection in characters.
    pub length: usize,
    /// Executable name of the app the text came from.
    pub source_app: String,
    /// True when the text must not be sent.
    pub blocked: bool,
    /// Why it is blocked.
    pub reason: Option<String>,
}

/// Builds the event payload from a capture. Secret-looking text is checked on the whole selection
/// and never put in the preview.
pub fn analyze(text: Option<&str>, source_app: &str) -> Captured {
    let text = text.unwrap_or("");
    let secret = looks_secret(text).is_some();
    Captured {
        preview: if secret {
            String::new()
        } else {
            text.chars().take(PREVIEW_CHARS).collect()
        },
        length: text.chars().count(),
        source_app: source_app.to_owned(),
        blocked: secret,
        reason: secret
            .then(|| "The text looks like a secret or API key, so it was not captured.".to_owned()),
    }
}

/// The Explain feature.
#[derive(Default)]
pub struct Explain;

#[async_trait]
impl Feature for Explain {
    fn id(&self) -> &'static str {
        "explain"
    }
    fn title(&self) -> &'static str {
        "Explain this"
    }
    fn version_tag(&self) -> &'static str {
        "v0.1.0"
    }
    fn description(&self) -> &'static str {
        "A second hotkey sends the selected text to Query mode. You see it before anything is sent."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema { fields: vec![] }
    }
    fn start(&self, _ctx: FeatureCtx) -> Result<(), FeatureError> {
        RUNNING.store(true, Ordering::Release);
        Ok(())
    }
    fn stop(&self) {
        RUNNING.store(false, Ordering::Release);
    }
    fn query(&self, _input: &Input) -> Vec<Item> {
        Vec::new()
    }
    async fn execute(&self, _item: &Item, _action: Action) -> Result<Outcome, FeatureError> {
        Err(FeatureError::Other("unsupported action".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_a_key_and_hides_it() {
        let key = format!("sk-{}", "aB3xK9mQ".repeat(5));
        let c = analyze(Some(&format!("here: {key}")), "code.exe");
        assert!(c.blocked && c.preview.is_empty() && c.reason.is_some());
        assert!(c.length > 40);
    }

    #[test]
    fn blocks_a_long_high_entropy_token() {
        let c = analyze(Some("aB3xK9mQ2vL7nP4wR8tY6uI1oE5zC0dF"), "x.exe");
        assert!(c.blocked);
    }

    #[test]
    fn allows_prose_and_truncates_the_preview() {
        let prose = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let c = analyze(Some(&prose), "notepad.exe");
        assert!(!c.blocked && c.reason.is_none());
        assert_eq!(c.preview.chars().count(), PREVIEW_CHARS);
        assert_eq!(c.length, prose.chars().count());
        assert_eq!(c.source_app, "notepad.exe");
    }

    #[test]
    fn nothing_selected_is_empty_not_blocked() {
        let c = analyze(None, "a.exe");
        assert!(!c.blocked && c.length == 0 && c.preview.is_empty());
    }

    #[test]
    fn running_follows_start_and_stop() {
        Explain.stop();
        assert!(!is_running());
        RUNNING.store(true, Ordering::Release);
        Explain.stop();
        assert!(!is_running());
    }
}
