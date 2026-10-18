//! OpenAI-compatible chat completions client (`/chat/completions`, SSE).

use crate::http;
use crate::sse::SseEvent;
use crate::stream::{self, WireState};
use crate::{
    Delta, Effort, KeySource, Message, ProbeResult, Provider, ProviderConfig, ProviderError,
    Request, Role, Usage,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

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
        stream::list_models(authorize(
            &self.cfg,
            &*self.keys,
            self.http.get(self.url("models")),
        )?)
        .await
    }
}

/// Set the sensitive bearer header when a key exists.
fn authorize(
    cfg: &ProviderConfig,
    keys: &dyn KeySource,
    rb: reqwest::RequestBuilder,
) -> Result<reqwest::RequestBuilder, ProviderError> {
    let Some(key) = http::authorize(cfg, keys)? else {
        return Ok(rb);
    };
    let value = http::sensitive(&format!("Bearer {}", key.expose_secret()))?;
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
    if cfg.supports_effort {
        body["reasoning_effort"] = match req.effort {
            Effort::Fast => "low",
            Effort::Think => "high",
        }
        .into();
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
        let rb = authorize(
            &self.cfg,
            &*self.keys,
            self.http
                .post(self.url("chat/completions"))
                .json(&build_body(&self.cfg, &req)),
        );
        stream::start(rb, Wire::default(), cancel)
    }

    async fn probe(&self) -> ProbeResult {
        self.probe_with(Duration::from_secs(3)).await
    }
}
