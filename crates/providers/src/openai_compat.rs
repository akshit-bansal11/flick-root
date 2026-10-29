//! OpenAI-compatible chat completions client (`/chat/completions`, SSE).

use crate::http;
use crate::sse::SseEvent;
use crate::stream::{self, WireState};
use crate::{
    Delta, KeySource, MAX_FETCHED_MODELS, Message, ModelInfo, ProbeResult, Provider,
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
use zeroize::Zeroizing;

const SYSTEM_PROMPT: &str = "Answer directly and briefly. Markdown is allowed.";

/// Client for any server speaking the OpenAI chat completions wire.
pub struct OpenAiCompat {
    cfg: ProviderConfig,
    http: reqwest::Client,
    keys: Arc<dyn KeySource>,
}

impl OpenAiCompat {
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

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.cfg.base_url.as_str().trim_end_matches('/'),
            path
        )
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let rb = authorize(&self.cfg, &self.keys, self.http.get(self.url("models"))).await?;
        stream::list_models(rb).await
    }

    async fn fetch_model_list(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let rb = authorize(&self.cfg, &self.keys, self.http.get(self.url("models"))).await?;
        let v = stream::get_json(rb).await?;
        Ok(parse_models(&v, self.cfg.supports_effort))
    }
}

/// Model ids from a `GET /models` body in API order. The wire reports no effort support,
/// so every model gets low/medium/high when the preset can send effort, else none.
// ponytail: cap, a picker if users need more
pub(crate) fn parse_models(v: &Value, supports_effort: bool) -> Vec<ModelInfo> {
    let efforts: Vec<String> = if supports_effort {
        ["low", "medium", "high"].map(str::to_owned).to_vec()
    } else {
        Vec::new()
    };
    v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["id"].as_str())
                .take(MAX_FETCHED_MODELS)
                .map(|id| ModelInfo {
                    id: id.to_owned(),
                    efforts: efforts.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Set the sensitive bearer header when a key exists.
async fn authorize(
    cfg: &ProviderConfig,
    keys: &Arc<dyn KeySource>,
    rb: reqwest::RequestBuilder,
) -> Result<reqwest::RequestBuilder, ProviderError> {
    let Some(key) = http::authorize(cfg, keys).await? else {
        return Ok(rb);
    };
    let value = http::sensitive(&Zeroizing::new(format!("Bearer {}", key.expose_secret())))?;
    Ok(rb.header(reqwest::header::AUTHORIZATION, value))
}

fn role_str(r: Role) -> &'static str {
    match r {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn build_body(cfg: &ProviderConfig, req: &Request) -> Value {
    let mut msgs = vec![json!({"role": "system", "content": SYSTEM_PROMPT})];
    msgs.extend(
        req.messages
            .iter()
            .map(|Message { role, content }| json!({"role": role_str(*role), "content": content})),
    );
    let mut body = json!({
        "model": cfg.model, "messages": msgs, "stream": true, "max_tokens": req.max_tokens
    });
    if cfg.supports_effort
        && let Some(level) = &req.effort
    {
        body["reasoning_effort"] = level.as_str().into();
    }
    body
}

#[derive(Default)]
struct Wire {
    usage: Usage,
    thinking: bool,
}

impl WireState for Wire {
    const ENDED: &'static str = "stream ended without [DONE]";

    fn handle(&mut self, ev: SseEvent) -> Vec<Result<Delta, ProviderError>> {
        if ev.data.trim() == "[DONE]" {
            return vec![Ok(Delta::Done(self.usage))];
        }
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else {
            return vec![Err(ProviderError::Decode("event json"))];
        };
        let mut out = Vec::new();
        if let Some(u) = v.get("usage").filter(|u| !u.is_null())
            && let Ok(u) = serde_json::from_value::<Usage>(u.clone())
        {
            self.usage = u;
        }
        let d = &v["choices"][0]["delta"];
        let reasoning = ["reasoning_content", "reasoning"]
            .iter()
            .any(|k| d[*k].as_str().is_some_and(|s| !s.is_empty()));
        if reasoning && !self.thinking {
            self.thinking = true;
            out.push(Ok(Delta::Thinking));
        }
        if let Some(t) = d["content"].as_str().filter(|s| !s.is_empty()) {
            self.thinking = false;
            out.push(Ok(Delta::Text(t.to_owned())));
        }
        out
    }
}

#[async_trait]
impl Provider for OpenAiCompat {
    fn config(&self) -> &ProviderConfig {
        &self.cfg
    }

    fn stream(
        &self,
        req: Request,
        cancel: CancellationToken,
    ) -> BoxStream<'static, Result<Delta, ProviderError>> {
        let (cfg, keys, http) = (self.cfg.clone(), Arc::clone(&self.keys), self.http.clone());
        let post = self.url("chat/completions");
        futures::stream::once(async move {
            let rb = authorize(&cfg, &keys, http.post(post).json(&build_body(&cfg, &req))).await;
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

    #[test]
    fn ids_efforts_and_cap() {
        let v = json!({ "data": [{ "id": "a" }, { "object": "x" }, { "id": "b" }] });
        let got = parse_models(&v, true);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].id, "b");
        assert_eq!(got[0].efforts, ["low", "medium", "high"]);
        assert!(parse_models(&v, false).iter().all(|m| m.efforts.is_empty()));
        let data: Vec<_> = (0..30).map(|i| json!({ "id": format!("m{i}") })).collect();
        assert_eq!(
            parse_models(&json!({ "data": data }), false).len(),
            crate::MAX_FETCHED_MODELS
        );
        assert!(parse_models(&json!({}), true).is_empty());
    }
}
