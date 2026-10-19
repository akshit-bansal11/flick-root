//! `DecisionEngine` trait and the `/v1/systemone` client for Laya, Kev, Jev and Custom.
//!
//! Protocol (docs.typesafe.ai/api.md): POST `{state, model, questions}` with a Bearer key;
//! the reply holds `answers.<qid>` of `type: "choice"` with `choice` (the option key),
//! `probabilities` and `confidence`. No health endpoint is documented, so `probe` only
//! checks that the base URL answers over HTTP. The Laya README was not reachable from those docs.

use crate::{ChoiceAnswer, ChoiceQuestion, EngineError, ProbeResult};
use async_trait::async_trait;
use providers::http::check_url;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;
use url::Url;

/// Response body cap.
const MAX_BODY: usize = 64 * 1024;
const CONNECT: Duration = Duration::from_secs(2);
const QID: &str = "pick";

/// A decision engine that answers a Choice question (ADR-005).
#[async_trait]
pub trait DecisionEngine: Send + Sync {
    /// "laya", "kev", "llm", "jev" or "custom".
    fn id(&self) -> &'static str;
    /// Pick one candidate.
    async fn choose(&self, q: &ChoiceQuestion) -> Result<ChoiceAnswer, EngineError>;
    /// Cheap reachability check.
    async fn probe(&self) -> ProbeResult;
}

#[derive(Serialize)]
struct WireState<'a> {
    phrase: &'a str,
}
#[derive(Serialize)]
struct WireQuestion {
    r#type: &'static str,
    instructions: &'static str,
    criteria: BTreeMap<String, String>,
}
/// Request body; carries only the phrase and candidate name + parent folder.
#[derive(Serialize)]
struct WireRequest<'a> {
    state: WireState<'a>,
    model: String,
    questions: BTreeMap<&'static str, WireQuestion>,
}
#[derive(Deserialize)]
struct WireAnswer {
    choice: String,
    #[serde(default)]
    probabilities: BTreeMap<String, f32>,
    #[serde(default)]
    confidence: Option<f32>,
}
#[derive(Deserialize)]
struct WireResponse {
    answers: BTreeMap<String, WireAnswer>,
}

fn build_request<'a>(id: &str, q: &'a ChoiceQuestion) -> WireRequest<'a> {
    let criteria = q
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i.to_string(), format!("{} in {}", c.name, c.parent)))
        .collect();
    let question = WireQuestion {
        r#type: "choice",
        instructions: "Pick the file the user's phrase most likely describes.",
        criteria,
    };
    WireRequest {
        state: WireState { phrase: &q.phrase },
        model: format!("{id}-latest"),
        questions: BTreeMap::from([(QID, question)]),
    }
}

fn parse_answer(id: &'static str, n: usize, body: &[u8]) -> Result<ChoiceAnswer, EngineError> {
    let bad = |m: &str| EngineError::BadResponse(m.to_owned());
    let resp: WireResponse = serde_json::from_slice(body).map_err(|_| bad("malformed json"))?;
    let a = resp.answers.get(QID).ok_or_else(|| bad("missing answer"))?;
    let index: usize = a.choice.parse().map_err(|_| bad("choice not an index"))?;
    if index >= n {
        return Err(bad("index out of range"));
    }
    let p = a
        .probabilities
        .get(&a.choice)
        .copied()
        .or(a.confidence)
        .ok_or_else(|| bad("no probability"))?;
    if !(0.0..=1.0).contains(&p) {
        return Err(bad("p out of range"));
    }
    Ok(ChoiceAnswer {
        index,
        p,
        engine: id,
    })
}

/// Client for one `/v1/systemone` engine.
#[derive(Debug)]
pub struct SystemOneEngine {
    /// "laya", "kev", "jev" or "custom".
    pub id: &'static str,
    /// Server base URL.
    pub base_url: Url,
    /// Host the bearer key is bound to (ADR-011); the key is never sent elsewhere.
    pub key_host: Option<String>,
    /// Bearer key, if the server needs one.
    pub bearer: Option<SecretString>,
    /// Total request timeout.
    pub timeout: Duration,
}

impl SystemOneEngine {
    /// No redirects (a key must not follow one), 2 s connect timeout.
    fn client(total: Duration) -> Result<reqwest::Client, EngineError> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT)
            .timeout(total)
            .build()
            .map_err(|_| EngineError::Unreachable("http client build failed".into()))
    }

    fn map_err(e: reqwest::Error) -> EngineError {
        if e.is_timeout() {
            EngineError::Timeout
        } else {
            EngineError::Unreachable(
                if e.is_connect() {
                    "connect failed"
                } else {
                    "request failed"
                }
                .into(),
            )
        }
    }
}

#[async_trait]
impl DecisionEngine for SystemOneEngine {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn choose(&self, q: &ChoiceQuestion) -> Result<ChoiceAnswer, EngineError> {
        let url = self
            .base_url
            .join("/v1/systemone")
            .map_err(|_| EngineError::Unreachable("bad base url".into()))?;
        let mut req = Self::client(self.timeout)?
            .post(url.clone())
            .json(&build_request(self.id, q));
        // The phrase and file names must not go over plain http to a remote either.
        let bound = self
            .key_host
            .as_deref()
            .or_else(|| url.host_str())
            .unwrap_or_default();
        check_url(&url, bound).map_err(|e| EngineError::Unreachable(e.to_string()))?;
        if let Some(key) = &self.bearer {
            let mut v =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", key.expose_secret()))
                    .map_err(|_| EngineError::Unreachable("key not header-safe".into()))?;
            v.set_sensitive(true);
            req = req.header(reqwest::header::AUTHORIZATION, v);
        }
        let mut resp = req.send().await.map_err(Self::map_err)?;
        if !resp.status().is_success() {
            return Err(EngineError::BadResponse(format!(
                "status {}",
                resp.status().as_u16()
            )));
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(Self::map_err)? {
            if buf.len() + chunk.len() > MAX_BODY {
                return Err(EngineError::BadResponse("response too large".into()));
            }
            buf.extend_from_slice(&chunk);
        }
        parse_answer(self.id, q.candidates.len(), &buf)
    }

    async fn probe(&self) -> ProbeResult {
        let res = match Self::client(CONNECT) {
            Ok(c) => c
                .get(self.base_url.clone())
                .send()
                .await
                .map(|_| ())
                .map_err(Self::map_err),
            Err(e) => Err(e),
        };
        ProbeResult {
            id: self.id.to_owned(),
            reachable: res.is_ok(),
            detail: res.err().map(|e| e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Candidate;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn q() -> ChoiceQuestion {
        let c = |n: &str| Candidate {
            name: n.into(),
            parent: "C:\\docs".into(),
        };
        ChoiceQuestion {
            phrase: "my cv".into(),
            candidates: vec![c("a.pdf"), c("cv.pdf")],
        }
    }
    fn eng(s: &MockServer, key: bool, host: &str) -> SystemOneEngine {
        SystemOneEngine {
            id: "laya",
            base_url: Url::parse(&s.uri()).unwrap(),
            key_host: Some(host.into()),
            bearer: key.then(|| SecretString::from("k3y")),
            timeout: Duration::from_millis(400),
        }
    }
    async fn serve(t: ResponseTemplate) -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(t)
            .mount(&s)
            .await;
        s
    }
    fn ok(choice: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "laya-latest",
            "answers": {"pick": {"type": "choice", "choice": choice, "probabilities": {"1": 0.9}, "confidence": 0.8}},
            "usage": {"input_tokens": 1, "output_tokens": 1}
        }))
    }
    fn is_bad(r: Result<ChoiceAnswer, EngineError>) -> bool {
        matches!(r, Err(EngineError::BadResponse(_)))
    }

    #[tokio::test]
    async fn good_answer_with_bearer() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("authorization", "Bearer k3y"))
            .respond_with(ok("1"))
            .mount(&s)
            .await;
        let a = eng(&s, true, "127.0.0.1").choose(&q()).await.unwrap();
        assert_eq!((a.index, a.engine), (1, "laya"));
        assert!((a.p - 0.9).abs() < 1e-6);
    }
    #[tokio::test]
    async fn out_of_range_index() {
        let s = serve(ok("5")).await;
        assert!(is_bad(eng(&s, false, "127.0.0.1").choose(&q()).await));
    }
    #[tokio::test]
    async fn malformed_json() {
        let s = serve(ResponseTemplate::new(200).set_body_string("{nope")).await;
        assert!(is_bad(eng(&s, false, "127.0.0.1").choose(&q()).await));
    }
    #[tokio::test]
    async fn timeout() {
        let s = serve(ok("1").set_delay(Duration::from_secs(2))).await;
        let r = eng(&s, false, "127.0.0.1").choose(&q()).await;
        assert!(matches!(r, Err(EngineError::Timeout)));
    }
    #[tokio::test]
    async fn server_error() {
        let s = serve(ResponseTemplate::new(500)).await;
        assert!(is_bad(eng(&s, false, "127.0.0.1").choose(&q()).await));
    }
    #[tokio::test]
    async fn oversize_body() {
        let s = serve(ResponseTemplate::new(200).set_body_bytes(vec![b' '; MAX_BODY + 1])).await;
        assert!(is_bad(eng(&s, false, "127.0.0.1").choose(&q()).await));
    }
    #[tokio::test]
    async fn host_binding_refusal() {
        let s = serve(ok("1")).await;
        let e = eng(&s, true, "other.example").choose(&q()).await;
        assert!(matches!(e, Err(EngineError::Unreachable(_))));
        assert!(s.received_requests().await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn plain_http_remote_refused_even_without_key() {
        let e = SystemOneEngine {
            id: "laya",
            base_url: Url::parse("http://example.invalid").unwrap(),
            key_host: None,
            bearer: None,
            timeout: Duration::from_millis(400),
        };
        let r = e.choose(&q()).await;
        assert!(matches!(r, Err(EngineError::Unreachable(m)) if m.contains("http")));
    }
    #[tokio::test]
    async fn probe_reachable() {
        let s = MockServer::start().await;
        assert!(eng(&s, false, "127.0.0.1").probe().await.reachable);
    }
    #[test]
    fn request_has_only_allowed_fields() {
        let v = serde_json::to_value(build_request("kev", &q())).unwrap();
        assert_eq!(v["model"], "kev-latest");
        assert_eq!(v["state"], serde_json::json!({"phrase": "my cv"}));
        assert_eq!(v.as_object().unwrap().len(), 3);
        let qn = &v["questions"]["pick"];
        assert_eq!(qn.as_object().unwrap().len(), 3);
        assert_eq!(
            qn["criteria"],
            serde_json::json!({"0": "a.pdf in C:\\docs", "1": "cv.pdf in C:\\docs"})
        );
    }
    #[test]
    fn debug_redacts_key() {
        let e = SystemOneEngine {
            id: "jev",
            base_url: Url::parse("https://x.test").unwrap(),
            key_host: None,
            bearer: Some(SecretString::from("k3y")),
            timeout: Duration::from_secs(1),
        };
        assert!(!format!("{e:?}").contains("k3y"));
    }
}
