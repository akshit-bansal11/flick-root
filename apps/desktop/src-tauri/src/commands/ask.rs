//! The `ask_stream` and `cancel_ask` commands (Query mode).
use crate::error::CommandError;
use crate::state::AppState;
use futures::StreamExt;
use launcher_core::detect::looks_secret;
use parking_lot::Mutex;
use providers::hub::HubEvent;
use providers::{Effort, Message, Request, Role};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use tauri::State;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

/// Events streamed to the UI; mirrors the TS `AskEvent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AskEvent {
    /// First provider selected.
    Started {
        /// Provider id.
        provider: String,
        /// Model name.
        model: String,
    },
    /// Moved on to the next provider.
    Fallback {
        /// Failed provider id.
        from: String,
        /// Next provider id.
        to: String,
        /// Short reason.
        reason: String,
    },
    /// The model is reasoning.
    Thinking,
    /// Answer text.
    Delta {
        /// Text chunk.
        text: String,
    },
    /// Finished.
    Done {
        /// Provider that answered.
        provider: String,
    },
    /// Failed.
    Error {
        /// Machine code.
        code: String,
        /// Human message, no secrets.
        message: String,
    },
}

impl From<HubEvent> for AskEvent {
    fn from(e: HubEvent) -> Self {
        match e {
            HubEvent::Started { provider, model } => Self::Started { provider, model },
            HubEvent::Fallback { from, to, reason } => Self::Fallback { from, to, reason },
            HubEvent::Thinking => Self::Thinking,
            HubEvent::Delta { text } => Self::Delta { text },
            HubEvent::Done { provider } => Self::Done { provider },
            HubEvent::Error { code, message } => Self::Error { code, message },
        }
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// In-flight asks by id; an entry lives until its task ends.
static ASKS: LazyLock<Mutex<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn error(code: &str, message: &str) -> AskEvent {
    AskEvent::Error {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// The event that refuses the ask before any provider is contacted, if any.
fn refusal(text: &str, attachment: Option<&str>, no_provider: bool) -> Option<AskEvent> {
    if looks_secret(text).is_some() || attachment.is_some_and(|a| looks_secret(a).is_some()) {
        return Some(error("secret_blocked", "the text looks like a secret"));
    }
    no_provider.then(|| error("no_provider", "no provider is enabled"))
}

/// Query mode gate: errors with `mode_disabled` when the mode is off.
fn check_enabled(enabled: bool) -> Result<(), CommandError> {
    if enabled {
        Ok(())
    } else {
        Err(CommandError::new(
            "mode_disabled",
            "Ask is off. Turn it on in Settings, Prefixes.",
        ))
    }
}

fn request(text: &str, attachment: Option<&str>, effort: &str) -> Request {
    let content = match attachment {
        Some(a) if !a.is_empty() => format!("{text}\n\n{a}"),
        _ => text.to_owned(),
    };
    let effort = if effort == "think" {
        Effort::Think
    } else {
        Effort::Fast
    };
    Request::new(
        vec![Message {
            role: Role::User,
            content,
        }],
        effort,
    )
}

/// Streams an answer over `on_event`; returns the ask id for `cancel_ask`.
#[tauri::command]
pub async fn ask_stream(
    state: State<'_, Arc<AppState>>,
    text: String,
    attachment: Option<String>,
    effort: String,
    thinking_mode: Option<String>,
    on_event: Channel<AskEvent>,
) -> Result<String, CommandError> {
    check_enabled(state.config.load().modes.query.enabled)?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let hub = state.ctx.providers.load_full();
    if let Some(e) = refusal(&text, attachment.as_deref(), hub.is_empty()) {
        if let Err(err) = on_event.send(e) {
            tracing::warn!(%err, "ask event send failed");
        }
        return Ok(id);
    }
    let cancel = CancellationToken::new();
    ASKS.lock().insert(id.clone(), cancel.clone());
    let first = {
        let cfg = state.config.load();
        thinking_mode
            .and_then(|m| super::providers::resolve_mode(&cfg, &m))
            .and_then(|(p, m)| hub.with_model(&p, &m))
    };
    let mut events = hub.ask_with(
        request(&text, attachment.as_deref(), &effort),
        cancel.clone(),
        first,
    );
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = events.next().await {
            if on_event.send(ev.into()).is_err() {
                cancel.cancel();
                break;
            }
        }
        ASKS.lock().remove(&task_id);
    });
    Ok(id)
}

/// Cancels an in-flight ask; an unknown id is a no-op.
#[tauri::command]
pub fn cancel_ask(ask_id: String) -> Result<(), CommandError> {
    if let Some(t) = ASKS.lock().get(&ask_id) {
        t.cancel();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_events() {
        let e: AskEvent = HubEvent::Delta { text: "hi".into() }.into();
        assert_eq!(e, AskEvent::Delta { text: "hi".into() });
        assert_eq!(AskEvent::from(HubEvent::Thinking), AskEvent::Thinking);
        let v = serde_json::to_value(AskEvent::Thinking).unwrap();
        assert_eq!(v, serde_json::json!({"type": "thinking"}));
    }

    #[test]
    fn refuses_secret_and_empty_hub() {
        let key = concat!(
            "sk-a",
            "nt-api03-abcdefghijklmnopqrstuvwxyz0123456789ABCDEF"
        );
        let code = |e: Option<AskEvent>| match e {
            Some(AskEvent::Error { code, .. }) => code,
            _ => String::new(),
        };
        assert_eq!(code(refusal("hello", Some(key), false)), "secret_blocked");
        assert_eq!(code(refusal(key, None, true)), "secret_blocked");
        assert_eq!(code(refusal("hello", None, true)), "no_provider");
        assert!(refusal("hello", None, false).is_none());
    }

    #[test]
    fn refuses_a_plain_sk_key_in_the_attachment() {
        let key = format!("sk-{}", "aB3xK9mQ".repeat(5));
        assert!(matches!(
            refusal("Explain this", Some(&key), false),
            Some(AskEvent::Error { code, .. }) if code == "secret_blocked"
        ));
    }

    #[test]
    fn query_gate() {
        assert!(check_enabled(true).is_ok());
        assert_eq!(check_enabled(false).unwrap_err().code, "mode_disabled");
    }

    #[test]
    fn cancel_unknown_is_ok() {
        assert!(cancel_ask("nope".into()).is_ok());
    }
}
