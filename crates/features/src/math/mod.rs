//! Inline math, units and time zones (F02). Currency arrives with T4.4.
//!
//! Contract: the item's `payload.value` is the plain result text. On `CopyResult` the
//! shell copies `payload.value` to the clipboard; `execute` only answers `Outcome::Copied`.

mod calc;
mod zones;

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};

/// Inline math and units.
pub struct Math;

/// Answers `text` as a time-zone conversion first, then as math or units.
fn answer(text: &str, now: DateTime<Utc>) -> Option<String> {
    zones::convert(text, now).or_else(|| calc::evaluate(text))
}

#[async_trait]
impl Feature for Math {
    fn id(&self) -> &'static str {
        "math"
    }
    fn title(&self) -> &'static str {
        "Inline math and units"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Results while you type in Command mode: math, units, time zones and currency."
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
        Ok(())
    }
    fn stop(&self) {}
    fn query(&self, input: &Input) -> Vec<Item> {
        let text = input.text.trim();
        // Cheap reject before touching the clock or fend.
        if !text.bytes().any(|b| b.is_ascii_digit()) && !text.to_lowercase().starts_with("now ") {
            return Vec::new();
        }
        match answer(text, Utc::now()) {
            Some(value) => vec![Item {
                id: ItemId(format!("math:{text}")),
                kind: ItemKind::Result,
                title: value.clone(),
                subtitle: text.split_whitespace().collect::<Vec<_>>().join(" "),
                icon: None,
                score: 1000.0,
                source: "math".into(),
                actions: vec![ActionKind::CopyResult],
                payload: serde_json::json!({ "value": value }),
            }],
            None => Vec::new(),
        }
    }
    async fn execute(&self, _item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        match action.kind {
            ActionKind::CopyResult => Ok(Outcome::Copied),
            _ => Err(FeatureError::Other("unsupported action".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(t: &str) -> Input {
        Input {
            raw: t.into(),
            text: t.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    #[test]
    fn query_builds_one_result_item() {
        let items = Math.query(&input("2.5 kg to lb"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Result);
        assert!(items[0].title.starts_with("5.51"));
        assert_eq!(items[0].payload["value"], items[0].title.as_str());
        assert!(Math.query(&input("notepad")).is_empty());
        assert!(Math.query(&input("hello")).is_empty());
        assert!(Math.query(&input("2++")).is_empty());
    }

    #[tokio::test]
    async fn copy_result_answers_copied() {
        let item = &Math.query(&input("2+2*3"))[0];
        let a = Action {
            kind: ActionKind::CopyResult,
            confirm_token: None,
        };
        assert_eq!(Math.execute(item, a).await.ok(), Some(Outcome::Copied));
    }
}
