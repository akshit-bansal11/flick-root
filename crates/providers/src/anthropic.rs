//! Anthropic Messages client (`/v1/messages`, SSE).
//!
//! Haiku 5.5 rules (ADR-006): never send `temperature`, `top_p`, `top_k` or an
//! assistant prefill. Reasoning is steered with `output_config.effort` only;
//! adaptive thinking is the model default, so `thinking` is never sent, and
//! `budget_tokens` is rejected with a 400 on these models.

use crate::http;
use crate::sse::SseEvent;
use crate::stream::{self, WireState};
use crate::{
    Delta, EFFORT_LEVELS, KeySource, MAX_FETCHED_MODELS, ModelInfo, ProbeResult, Provider,
    ProviderConfig, ProviderError, Request, Role, Usage,
};
use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::BoxStream;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const SYSTEM_PROMPT: &str = "Answer directly and briefly. Markdown is allowed.";
/// Value of the `anthropic-version` header (per the Messages API docs).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Client for the Anthropic Messages API.
pub struct Anthropic {
    cfg: ProviderConfig,
    http: reqwest::Client,
    keys: Arc<dyn KeySource>,
}

impl Anthropic {
    /// Build a client with the no-redirect, 2 s connect-timeout authed HTTP client.
    pub fn new(cfg: ProviderConfig, keys: Arc<dyn KeySource>) -> Result<Self, ProviderError> {
        Ok(Self {
            cfg,
            http: http::authed_client()?,
            keys,
        })
    }

    /// Probe with a caller-given timeout.
    pub async fn probe_with(&self, timeout: Duration) -> ProbeResult {
        stream::probe(&self.cfg.id, timeout, self.list_models()).await
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        // The key is read once, inside `authorize`; no key means no probe.
        let rb = authorize(
            &self.cfg,
            &self.keys,
            self.http.get(url(&self.cfg, "v1/models")),
            true,
        )
        .await?;
        stream::list_models(rb).await
    }

    async fn fetch_model_list(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let rb = authorize(
            &self.cfg,
            &self.keys,
            self.http.get(url(&self.cfg, "v1/models?limit=1000")),
            true,
        )
        .await?;
        Ok(parse_models(&stream::get_json(rb).await?))
    }
}

/// Models from a `GET /v1/models` body: active (or lifecycle-less) entries in API order,
/// each with the effort levels its `capabilities.effort` marks supported. Missing or null
/// `capabilities`, `effort` or level objects mean "not supported".
// ponytail: first page only (limit=1000), cap 20; add paging/picker if users need more
pub(crate) fn parse_models(v: &Value) -> Vec<ModelInfo> {
    let Some(data) = v["data"].as_array() else {
        return Vec::new();
    };
    data.iter()
        .filter(|m| m["lifecycle"].as_str().is_none_or(|l| l == "active"))
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_owned();
            let effort = &m["capabilities"]["effort"];
            let on = |k: &str| effort["supported"] == true && effort[k]["supported"] == true;
            let efforts = EFFORT_LEVELS
                .iter()
                .filter(|l| on(l))
                .map(|l| (*l).to_owned())
                .collect();
            Some(ModelInfo { id, efforts })
        })
        .take(MAX_FETCHED_MODELS)
        .collect()
}

fn host_of(cfg: &ProviderConfig) -> String {
    cfg.base_url.host_str().unwrap_or_default().to_owned()
}

fn url(cfg: &ProviderConfig, path: &str) -> String {
    format!("{}/{}", cfg.base_url.as_str().trim_end_matches('/'), path)
}

/// Set the version header and, when a key exists, the sensitive `x-api-key` header.
/// With `require_key`, a missing key is `NoKey`.
async fn authorize(
    cfg: &ProviderConfig,
    keys: &Arc<dyn KeySource>,
    rb: reqwest::RequestBuilder,
    require_key: bool,
) -> Result<reqwest::RequestBuilder, ProviderError> {
    let rb = rb.header("anthropic-version", ANTHROPIC_VERSION);
    let Some(key) = http::authorize(cfg, keys).await? else {
        return if require_key {
            Err(ProviderError::NoKey { host: host_of(cfg) })
        } else {
            Ok(rb)
        };
    };
    Ok(rb.header("x-api-key", http::sensitive(key.expose_secret())?))
}

fn build_body(cfg: &ProviderConfig, req: &Request) -> Result<Value, ProviderError> {
    let msgs: Vec<Value> = req
        .messages
        .iter()
        .filter_map(|m| match m.role {
            Role::User => Some(json!({"role": "user", "content": m.content})),
            Role::Assistant => Some(json!({"role": "assistant", "content": m.content})),
            Role::System => None,
        })
        .collect();
    // Never send an assistant prefill: the last turn must be the user's.
    if !req.messages.last().is_some_and(|m| m.role == Role::User) {
        return Err(ProviderError::Decode("last message must be from the user"));
    }
    let mut body = json!({
        "model": cfg.model, "max_tokens": req.max_tokens, "system": SYSTEM_PROMPT,
        "messages": msgs, "stream": true
    });
    // Adaptive thinking is the model default: send only the chosen level, if any.
    // No budget_tokens: manual thinking is a 400 on Haiku 5.5.
    if cfg.supports_effort
        && let Some(level) = &req.effort
    {
        body["output_config"] = json!({"effort": level});
    }
    Ok(body)
}

#[derive(Default)]
struct Wire {
    usage: Usage,
    thinking: bool,
}

fn set_u32(dst: &mut u32, v: &Value) {
    if let Some(n) = v.as_u64() {
        *dst = u32::try_from(n).unwrap_or(u32::MAX);
    }
}

impl Wire {
    fn mark_thinking(&mut self, out: &mut Vec<Result<Delta, ProviderError>>) {
        if !self.thinking {
            self.thinking = true;
            out.push(Ok(Delta::Thinking));
        }
    }
}

impl WireState for Wire {
    const ENDED: &'static str = "stream ended without message_stop";

    fn handle(&mut self, ev: SseEvent) -> Vec<Result<Delta, ProviderError>> {
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else {
            return vec![Err(ProviderError::Decode("event json"))];
        };
        let mut out = Vec::new();
        match v["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let u = &v["message"]["usage"];
                set_u32(&mut self.usage.prompt_tokens, &u["input_tokens"]);
                set_u32(&mut self.usage.completion_tokens, &u["output_tokens"]);
            }
            "content_block_start" if v["content_block"]["type"] == "thinking" => {
                self.mark_thinking(&mut out);
            }
            "content_block_delta" => {
                let d = &v["delta"];
                match d["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        if let Some(t) = d["text"].as_str().filter(|s| !s.is_empty()) {
                            out.push(Ok(Delta::Text(t.to_owned())));
                        }
                    }
                    "thinking_delta" => self.mark_thinking(&mut out),
                    _ => {} // signature_delta and unknown deltas
                }
            }
            "content_block_stop" => self.thinking = false,
            "message_delta" => {
                let u = &v["usage"];
                set_u32(&mut self.usage.prompt_tokens, &u["input_tokens"]);
                set_u32(&mut self.usage.completion_tokens, &u["output_tokens"]);
            }
            "message_stop" => out.push(Ok(Delta::Done(self.usage))),
            "error" => {
                // Map to the HTTP status the same condition would carry; never keep the message.
                out.push(Err(match v["error"]["type"].as_str() {
                    Some("overloaded_error") => ProviderError::Status { code: 529 },
                    Some("api_error") => ProviderError::Status { code: 500 },
                    Some("rate_limit_error") => ProviderError::Status { code: 429 },
                    _ => ProviderError::Decode("stream error event"),
                }));
            }
            _ => {} // ping and unknown events
        }
        out
    }
}

#[async_trait]
impl Provider for Anthropic {
    fn config(&self) -> &ProviderConfig {
        &self.cfg
    }

    fn stream(
        &self,
        req: Request,
        cancel: CancellationToken,
    ) -> BoxStream<'static, Result<Delta, ProviderError>> {
        let (cfg, keys, http) = (self.cfg.clone(), Arc::clone(&self.keys), self.http.clone());
        futures::stream::once(async move {
            let rb = match build_body(&cfg, &req) {
                Ok(body) => {
                    let rb = http.post(url(&cfg, "v1/messages")).json(&body);
                    authorize(&cfg, &keys, rb, false).await
                }
                Err(e) => Err(e),
            };
            stream::start(rb, Wire::default(), cancel)
        })
        .flatten()
        .boxed()
    }

    async fn probe(&self) -> ProbeResult {
        self.probe_with(Duration::from_secs(3)).await
    }

    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        stream::with_deadline(self.fetch_model_list()).await
    }
}

#[cfg(test)]
mod tests {
    use super::parse_models;
    use serde_json::json;

    fn ids(v: &serde_json::Value) -> Vec<String> {
        parse_models(v).into_iter().map(|m| m.id).collect()
    }

    fn level(on: bool) -> serde_json::Value {
        json!({ "supported": on })
    }

    #[test]
    fn keeps_active_and_reads_efforts() {
        let v = json!({
            "data": [
                {
                    "id": "a",
                    "lifecycle": "active",
                    "capabilities": { "effort": {
                        "supported": true, "low": level(true), "medium": level(false),
                        "high": level(true), "xhigh": level(true), "max": level(true)
                    }}
                },
                { "id": "old", "lifecycle": "retired", "capabilities": null },
                { "id": "dep", "lifecycle": "deprecated", "capabilities": null },
                { "id": "b" }
            ],
            "has_more": false
        });
        let got = parse_models(&v);
        assert_eq!(ids(&v), ["a", "b"]);
        assert_eq!(got[0].efforts, ["low", "high", "xhigh", "max"]);
        assert!(got[1].efforts.is_empty());
    }

    #[test]
    fn null_capabilities_and_null_xhigh_skip_cleanly() {
        let v = json!({ "data": [
            { "id": "n", "lifecycle": "active", "capabilities": null },
            { "id": "x", "lifecycle": "active", "capabilities": { "effort": {
                "supported": true, "low": level(true), "medium": level(true),
                "high": level(true), "xhigh": null, "max": level(true)
            }}},
            { "id": "e", "capabilities": { "effort": null } }
        ]});
        let got = parse_models(&v);
        assert!(got[0].efforts.is_empty());
        assert_eq!(got[1].efforts, ["low", "medium", "high", "max"]);
        assert!(got[2].efforts.is_empty());
    }

    #[test]
    fn effort_unsupported_means_none() {
        let v = json!({ "data": [{ "id": "a", "capabilities": { "effort": {
            "supported": false, "low": level(true), "high": level(true)
        }}}]});
        assert!(parse_models(&v)[0].efforts.is_empty());
    }

    #[test]
    fn caps_at_twenty_and_tolerates_missing_data() {
        let data: Vec<_> = (0..25).map(|i| json!({ "id": format!("m{i}") })).collect();
        let got = ids(&json!({ "data": data }));
        assert_eq!(got.len(), crate::MAX_FETCHED_MODELS);
        assert_eq!(got[0], "m0");
        assert!(parse_models(&json!({})).is_empty());
        assert_eq!(
            ids(&json!({ "data": [{ "id": 3 }, { "id": "ok" }] })),
            ["ok"]
        );
    }
}
