//! Built-in provider presets. None is recommended and none is enabled by default (ADR-006).

use crate::{KeySource, OpenAiCompat, Provider, ProviderConfig, ProviderError, Wire};
use launcher_core::config::ProviderEntry;
use std::sync::Arc;
use url::Url;

/// Where a preset runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Server on this machine.
    Local,
    /// Free hosted gateway.
    FreeGateway,
    /// A vendor API the user brings a key for.
    OwnKey,
}

/// One built-in provider preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// Stable id, also the config table name.
    pub id: &'static str,
    /// Display title.
    pub title: &'static str,
    /// Group in the picker.
    pub group: Group,
    /// Wire protocol.
    pub wire: Wire,
    /// Default base URL, empty for custom.
    pub default_base_url: &'static str,
    /// Whether a key is required.
    pub key_required: bool,
    /// Whether `reasoning_effort` may be sent.
    pub supports_effort: bool,
}

impl Preset {
    /// Presets are never enabled by default.
    pub fn default_enabled(&self) -> bool {
        false
    }
}

const fn p(
    id: &'static str,
    title: &'static str,
    group: Group,
    wire: Wire,
    url: &'static str,
    key: bool,
    effort: bool,
) -> Preset {
    Preset {
        id,
        title,
        group,
        wire,
        default_base_url: url,
        key_required: key,
        supports_effort: effort,
    }
}

/// All presets in display and default fallback order.
pub const PRESETS: [Preset; 8] = [
    p(
        "ollama",
        "Ollama",
        Group::Local,
        Wire::OpenAi,
        "http://localhost:11434/v1",
        false,
        false,
    ),
    p(
        "lmstudio",
        "LM Studio",
        Group::Local,
        Wire::OpenAi,
        "http://localhost:1234/v1",
        false,
        false,
    ),
    p(
        "llamacpp",
        "llama.cpp",
        Group::Local,
        Wire::OpenAi,
        "http://localhost:8080/v1",
        false,
        false,
    ),
    p(
        "omniroute",
        "OmniRoute",
        Group::FreeGateway,
        Wire::OpenAi,
        "http://localhost:20128/v1",
        false,
        false,
    ),
    p(
        "openrouter",
        "OpenRouter",
        Group::FreeGateway,
        Wire::OpenAi,
        "https://openrouter.ai/api/v1",
        true,
        false,
    ),
    p(
        "anthropic",
        "Anthropic",
        Group::OwnKey,
        Wire::Anthropic,
        "https://api.anthropic.com",
        true,
        true,
    ),
    p(
        "openai",
        "OpenAI",
        Group::OwnKey,
        Wire::OpenAi,
        "https://api.openai.com/v1",
        true,
        true,
    ),
    p(
        "custom",
        "Custom",
        Group::OwnKey,
        Wire::OpenAi,
        "",
        false,
        false,
    ),
];

/// Look up a preset by id.
pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Build a provider from a config entry. Unknown ids use the custom preset.
pub fn build_provider(
    id: &str,
    entry: &ProviderEntry,
    preset: &Preset,
    keys: Arc<dyn KeySource>,
    http: reqwest::Client,
) -> Result<Box<dyn Provider>, ProviderError> {
    let base_url: Url = entry
        .base_url
        .parse()
        .map_err(|_| ProviderError::Decode("invalid base url"))?;
    let cfg = ProviderConfig {
        id: id.to_owned(),
        wire: preset.wire,
        base_url,
        model: entry.model.clone(),
        key_host: entry.key_host.clone(),
        supports_effort: preset.supports_effort,
    };
    match preset.wire {
        Wire::OpenAi => Ok(Box::new(OpenAiCompat::new(cfg, http, keys))),
        // TODO(T2.4): wire crate::anthropic::Anthropic::new(cfg, http, keys) once it exists.
        Wire::Anthropic => Err(ProviderError::Decode("anthropic wire not wired yet")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_defaults() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "ollama",
                "lmstudio",
                "llamacpp",
                "omniroute",
                "openrouter",
                "anthropic",
                "openai",
                "custom"
            ]
        );
        assert!(PRESETS.iter().all(|p| !p.default_enabled()));
        assert_eq!(preset("custom").map(|p| p.default_base_url), Some(""));
    }
}
