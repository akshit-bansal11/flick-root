//! Request, delta and configuration types shared by all providers.

use serde::{Deserialize, Serialize};
use url::Url;

/// Chat role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System prompt.
    System,
    /// User turn.
    User,
    /// Assistant turn.
    Assistant,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// Message text.
    pub content: String,
}

/// Effort levels a request may carry, ordered low to max.
pub const EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The level for a stored or wire effort string. Old `think` is `high`; `fast`, `auto`,
/// empty and unknown values are `None` (omit the parameter).
pub fn parse_effort(raw: &str) -> Option<&'static str> {
    let raw = raw.trim();
    if raw == "think" {
        return Some("high");
    }
    EFFORT_LEVELS.iter().copied().find(|l| *l == raw)
}

/// A completion request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Conversation so far (no system prompt; the client adds it).
    pub messages: Vec<Message>,
    /// Reasoning effort; `None` = auto, the parameter is omitted.
    pub effort: Option<String>,
    /// Output token cap.
    pub max_tokens: u32,
}

impl Request {
    /// Request with the default 1024 token cap.
    pub fn new(messages: Vec<Message>, effort: Option<String>) -> Self {
        Self {
            messages,
            effort,
            max_tokens: 1024,
        }
    }
}

/// Token usage reported at the end of a stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens.
    #[serde(default)]
    pub prompt_tokens: u32,
    /// Completion tokens.
    #[serde(default)]
    pub completion_tokens: u32,
}

/// One streamed item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// Answer text.
    Text(String),
    /// The model is reasoning (marker only, no text).
    Thinking,
    /// Stream finished.
    Done(Usage),
}

/// Wire protocol spoken by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wire {
    /// OpenAI chat completions.
    OpenAi,
    /// Anthropic messages.
    Anthropic,
}

/// Static description of one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Stable id.
    pub id: String,
    /// Wire protocol.
    pub wire: Wire,
    /// API base, e.g. `https://api.openai.com/v1`.
    pub base_url: Url,
    /// Model name.
    pub model: String,
    /// Host the key was entered for.
    pub key_host: Option<String>,
    /// Whether `reasoning_effort` may be sent.
    pub supports_effort: bool,
}

/// Outcome of a reachability probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Provider id.
    pub id: String,
    /// Whether the server answered successfully.
    pub reachable: bool,
    /// Model ids reported.
    pub models: Vec<String>,
    /// Round-trip milliseconds.
    pub ms: u64,
    /// Short error text, never containing secrets.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::parse_effort;

    #[test]
    fn parse_effort_maps_stored_values() {
        assert_eq!(parse_effort("think"), Some("high"));
        assert_eq!(parse_effort("fast"), None);
        assert_eq!(parse_effort("auto"), None);
        assert_eq!(parse_effort(""), None);
        assert_eq!(parse_effort("max"), Some("max"));
        assert_eq!(parse_effort("bogus"), None);
    }
}
