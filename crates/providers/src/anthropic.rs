//! Anthropic Messages client (`/v1/messages`, SSE).
//!
//! Haiku 5.5 rules (ADR-006): never send `temperature`, `top_p`, `top_k` or an
//! assistant prefill. Reasoning is steered with `output_config.effort` plus
//! adaptive thinking; `thinking.type = "enabled"` with `budget_tokens` is
//! rejected with a 400 on these models, so it is never sent.

use crate::http::{self, check_status, net_err, read_capped};
use crate::sse::{SseEvent, SseParser};
use crate::{
    Delta, Effort, KeySource, ProbeResult, Provider, ProviderConfig, ProviderError, Request, Role,
    Usage,
};
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const SYSTEM_PROMPT: &str = "Answer directly and briefly. Markdown is allowed.";
/// Value of the `anthropic-version` header (per the Messages API docs).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Cap for the `/v1/models` probe body.
const MAX_PROBE_BYTES: usize = 1 << 20;

/// Client for the Anthropic Messages API.
pub struct Anthropic {
    cfg: ProviderConfig,
    http: reqwest::Client,
    keys: Arc<dyn KeySource>,
}

impl Anthropic {
    /// Build a client; `http` should use a 5 s connect timeout and no overall timeout.
    pub fn new(cfg: ProviderConfig, http: reqwest::Client, keys: Arc<dyn KeySource>) -> Self {
        Self { cfg, http, keys }
    }

    /// Probe with a caller-given timeout.
    pub async fn probe_with(&self, timeout: Duration) -> ProbeResult {
        let start = Instant::now();
        let res = tokio::time::timeout(timeout, self.list_models()).await;
        let (reachable, models, error) = match res {
            Ok(Ok(m)) => (true, m, None),
            Ok(Err(e)) => (false, vec![], Some(e.to_string())),
            Err(_) => (false, vec![], Some("timed out".to_owned())),
        };
        ProbeResult {
            id: self.cfg.id.clone(),
            reachable,
            models,
            ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            error,
        }
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        if self.keys.key_for(&self.cfg)?.is_none() {
            return Err(ProviderError::NoKey {
                host: host_of(&self.cfg),
            });
        }
        let rb = authorize(
            &self.cfg,
            &*self.keys,
            self.http.get(url(&self.cfg, "v1/models")),
        )?;
        let resp = rb.send().await.map_err(net_err)?;
        check_status(&resp)?;
        let body = read_capped(resp, MAX_PROBE_BYTES).await?;
        let v: Value =
            serde_json::from_slice(&body).map_err(|_| ProviderError::Decode("models json"))?;
        Ok(v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn host_of(cfg: &ProviderConfig) -> String {
    cfg.base_url.host_str().unwrap_or_default().to_owned()
}

fn url(cfg: &ProviderConfig, path: &str) -> String {
    format!("{}/{}", cfg.base_url.as_str().trim_end_matches('/'), path)
}

/// Set the version header and, when a key exists, the sensitive `x-api-key` header.
fn authorize(
    cfg: &ProviderConfig,
    keys: &dyn KeySource,
    rb: reqwest::RequestBuilder,
) -> Result<reqwest::RequestBuilder, ProviderError> {
    let rb = rb.header("anthropic-version", ANTHROPIC_VERSION);
    let Some(key) = http::authorize(cfg, keys)? else {
        return Ok(rb);
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
    // Think: adaptive thinking at high effort. Fast: omit (model default).
    // No budget_tokens: manual thinking is a 400 on Haiku 5.5.
    if cfg.supports_effort && req.effort == Effort::Think {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": "high"});
    }
    Ok(body)
}

struct State {
    bytes: BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    parser: SseParser,
    pending: VecDeque<SseEvent>,
    cancel: CancellationToken,
    usage: Usage,
    thinking: bool,
    out: VecDeque<Delta>,
    done: bool,
}

fn set_u32(dst: &mut u32, v: &Value) {
    if let Some(n) = v.as_u64() {
        *dst = u32::try_from(n).unwrap_or(u32::MAX);
    }
}

impl State {
    /// Turn one event into zero or more deltas.
    fn handle(&mut self, ev: SseEvent) -> Result<(), ProviderError> {
        let v: Value =
            serde_json::from_str(&ev.data).map_err(|_| ProviderError::Decode("event json"))?;
        match v["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let u = &v["message"]["usage"];
                set_u32(&mut self.usage.prompt_tokens, &u["input_tokens"]);
                set_u32(&mut self.usage.completion_tokens, &u["output_tokens"]);
            }
            "content_block_start" if v["content_block"]["type"] == "thinking" => {
                self.mark_thinking()
            }
            "content_block_delta" => {
                let d = &v["delta"];
                match d["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        if let Some(t) = d["text"].as_str().filter(|s| !s.is_empty()) {
                            self.out.push_back(Delta::Text(t.to_owned()));
                        }
                    }
                    "thinking_delta" => self.mark_thinking(),
                    _ => {} // signature_delta and unknown deltas
                }
            }
            "content_block_stop" => self.thinking = false,
            "message_delta" => {
                let u = &v["usage"];
                set_u32(&mut self.usage.prompt_tokens, &u["input_tokens"]);
                set_u32(&mut self.usage.completion_tokens, &u["output_tokens"]);
            }
            "message_stop" => {
                self.out.push_back(Delta::Done(self.usage));
                self.done = true;
            }
            "error" => {
                // Map to the HTTP status the same condition would carry; never keep the message.
                return Err(match v["error"]["type"].as_str() {
                    Some("overloaded_error") => ProviderError::Status { code: 529 },
                    Some("api_error") => ProviderError::Status { code: 500 },
                    Some("rate_limit_error") => ProviderError::Status { code: 429 },
                    _ => ProviderError::Decode("stream error event"),
                });
            }
            _ => {} // ping and unknown events
        }
        Ok(())
    }

    fn mark_thinking(&mut self) {
        if !self.thinking {
            self.thinking = true;
            self.out.push_back(Delta::Thinking);
        }
    }

    async fn next(&mut self) -> Option<Result<Delta, ProviderError>> {
        loop {
            if self.cancel.is_cancelled() {
                if self.done {
                    return None;
                }
                self.done = true;
                self.out.clear();
                return Some(Err(ProviderError::Cancelled));
            }
            if let Some(d) = self.out.pop_front() {
                return Some(Ok(d));
            }
            if self.done {
                return None;
            }
            if let Some(ev) = self.pending.pop_front() {
                if let Err(e) = self.handle(ev) {
                    self.done = true;
                    return Some(Err(e));
                }
                continue;
            }
            let chunk = tokio::select! {
                () = self.cancel.cancelled() => continue,
                c = self.bytes.next() => c,
            };
            match chunk {
                Some(Ok(b)) => match self.parser.push(&b) {
                    Ok(evs) => self.pending.extend(evs),
                    Err(e) => {
                        self.done = true;
                        return Some(Err(e));
                    }
                },
                Some(Err(e)) => {
                    self.done = true;
                    return Some(Err(net_err(e)));
                }
                None => {
                    self.done = true;
                    return Some(Err(ProviderError::Decode(
                        "stream ended without message_stop",
                    )));
                }
            }
        }
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
        let http = self.http.clone();
        let cfg = self.cfg.clone();
        let keys = self.keys.clone();
        let endpoint = url(&cfg, "v1/messages");
        futures::stream::once(async move {
            let body = build_body(&cfg, &req)?;
            let rb = authorize(&cfg, &*keys, http.post(endpoint).json(&body))?;
            let resp = tokio::select! {
                () = cancel.cancelled() => return Err(ProviderError::Cancelled),
                r = rb.send() => r.map_err(net_err)?,
            };
            check_status(&resp)?;
            Ok(State {
                bytes: resp.bytes_stream().boxed(),
                parser: SseParser::default(),
                pending: VecDeque::new(),
                cancel,
                usage: Usage::default(),
                thinking: false,
                out: VecDeque::new(),
                done: false,
            })
        })
        .flat_map(|r| match r {
            Err(e) => futures::stream::once(async move { Err(e) }).boxed(),
            Ok(st) => futures::stream::unfold(st, |mut st| async move {
                let item = st.next().await?;
                Some((item, st))
            })
            .boxed(),
        })
        .boxed()
    }

    async fn probe(&self) -> ProbeResult {
        self.probe_with(Duration::from_secs(3)).await
    }
}
