//! OpenAI-compatible chat completions client (`/chat/completions`, SSE).

use crate::http::{self, check_status, net_err, read_capped};
use crate::sse::{SseEvent, SseParser};
use crate::{
    Delta, Effort, KeySource, Message, ProbeResult, Provider, ProviderConfig, ProviderError,
    Request, Role, Usage,
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

/// Client for any server speaking the OpenAI chat completions wire.
pub struct OpenAiCompat {
    cfg: ProviderConfig,
    http: reqwest::Client,
    keys: Arc<dyn KeySource>,
}

impl OpenAiCompat {
    /// Build a client; `http` should use a 5 s connect timeout and no overall timeout.
    pub fn new(cfg: ProviderConfig, http: reqwest::Client, keys: Arc<dyn KeySource>) -> Self {
        Self { cfg, http, keys }
    }

    /// Default HTTP client: 5 s connect timeout, no overall timeout.
    pub fn default_http() -> Result<reqwest::Client, ProviderError> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| ProviderError::Decode("http client build failed"))
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

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.cfg.base_url.as_str().trim_end_matches('/'),
            path
        )
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let rb = authorize(&self.cfg, &*self.keys, self.http.get(self.url("models")))?;
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

/// Cap for the `/models` probe body.
const MAX_PROBE_BYTES: usize = 1 << 20;

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

impl State {
    /// Turn one event into zero or more deltas.
    fn handle(&mut self, ev: SseEvent) -> Result<(), ProviderError> {
        if ev.data.trim() == "[DONE]" {
            self.out.push_back(Delta::Done(self.usage));
            self.done = true;
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(&ev.data).map_err(|_| ProviderError::Decode("event json"))?;
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
            self.out.push_back(Delta::Thinking);
        }
        if let Some(t) = d["content"].as_str().filter(|s| !s.is_empty()) {
            self.thinking = false;
            self.out.push_back(Delta::Text(t.to_owned()));
        }
        Ok(())
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
                    return Some(Err(ProviderError::Decode("stream ended without [DONE]")));
                }
            }
        }
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
        let http = self.http.clone();
        let cfg = self.cfg.clone();
        let keys = self.keys.clone();
        let url = self.url("chat/completions");
        futures::stream::once(async move {
            let rb = authorize(&cfg, &*keys, http.post(url).json(&build_body(&cfg, &req)))?;
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
