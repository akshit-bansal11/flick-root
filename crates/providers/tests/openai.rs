//! Wiremock tests for the OpenAI-compatible client. Keys here are fake.

use futures::StreamExt;
use providers::{
    Delta, Effort, KeySource, Message, NoKeys, OpenAiCompat, Provider, ProviderConfig,
    ProviderError, Request, Role, Wire,
};
use secrecy::SecretString;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct FakeKey;
impl KeySource for FakeKey {
    fn key_for(&self, _: &ProviderConfig) -> Result<Option<SecretString>, ProviderError> {
        Ok(Some(SecretString::from("sk-fake-0000".to_owned())))
    }
}

fn client(server: &MockServer, effort: bool, keys: Arc<dyn KeySource>) -> OpenAiCompat {
    let cfg = ProviderConfig {
        id: "t".into(),
        wire: Wire::OpenAi,
        base_url: format!("{}/v1", server.uri()).parse().unwrap(),
        model: "m".into(),
        key_host: None,
        supports_effort: effort,
    };
    OpenAiCompat::new(cfg, OpenAiCompat::default_http().unwrap(), keys)
}

fn req(effort: Effort) -> Request {
    Request::new(
        vec![Message {
            role: Role::User,
            content: "hi".into(),
        }],
        effort,
    )
}

fn sse(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_string(body)
}

async fn mount(server: &MockServer, t: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(t)
        .mount(server)
        .await;
}

async fn collect(p: &OpenAiCompat, e: Effort) -> Vec<Result<Delta, ProviderError>> {
    p.stream(req(e), CancellationToken::new()).collect().await
}

#[tokio::test]
async fn deltas_in_order() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/openai_basic.sse"))).await;
    let got: Vec<_> = collect(&client(&s, false, Arc::new(NoKeys)), Effort::Fast)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    let texts: Vec<_> = got
        .iter()
        .filter_map(|d| {
            if let Delta::Text(t) = d {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(texts, ["Hel", "lo", "!"]);
    let Some(Delta::Done(u)) = got.last() else {
        panic!("no Done")
    };
    assert_eq!((u.prompt_tokens, u.completion_tokens), (7, 3));
}

#[tokio::test]
async fn reasoning_yields_one_thinking() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/openai_reasoning.sse"))).await;
    let got: Vec<_> = collect(&client(&s, true, Arc::new(NoKeys)), Effort::Think)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(got.iter().filter(|d| **d == Delta::Thinking).count(), 1);
    assert_eq!(got[0], Delta::Thinking);
    assert_eq!(got[1], Delta::Text("Answer".into()));
}

#[tokio::test]
async fn cancel_after_first_delta() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/openai_basic.sse"))).await;
    let token = CancellationToken::new();
    let mut st = client(&s, false, Arc::new(NoKeys)).stream(req(Effort::Fast), token.clone());
    assert!(matches!(st.next().await, Some(Ok(Delta::Text(_)))));
    token.cancel();
    let t = Instant::now();
    assert!(matches!(
        st.next().await,
        Some(Err(ProviderError::Cancelled))
    ));
    assert!(st.next().await.is_none());
    assert!(t.elapsed() < Duration::from_millis(100));
}

#[tokio::test]
async fn status_errors() {
    for (code, fallback) in [(429, true), (503, true), (401, false)] {
        let s = MockServer::start().await;
        mount(
            &s,
            ResponseTemplate::new(code).set_body_string("secret body"),
        )
        .await;
        let got = collect(&client(&s, false, Arc::new(NoKeys)), Effort::Fast).await;
        let Some(Err(e)) = got.into_iter().next() else {
            panic!("expected error")
        };
        assert!(matches!(e, ProviderError::Status { code: c } if c == code));
        assert_eq!(e.is_fallback_worthy(), fallback, "{code}");
    }
}

async fn last_request(s: &MockServer) -> wiremock::Request {
    s.received_requests().await.unwrap().pop().unwrap()
}

#[tokio::test]
async fn effort_omitted_unless_supported() {
    for (supported, think, want) in [
        (false, Effort::Think, None),
        (true, Effort::Fast, Some("low")),
        (true, Effort::Think, Some("high")),
    ] {
        let s = MockServer::start().await;
        mount(&s, sse(include_str!("fixtures/openai_basic.sse"))).await;
        collect(&client(&s, supported, Arc::new(NoKeys)), think).await;
        let body: serde_json::Value = serde_json::from_slice(&last_request(&s).await.body).unwrap();
        assert_eq!(body["reasoning_effort"].as_str(), want);
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["role"], "system");
    }
}

#[tokio::test]
async fn auth_header_only_with_key() {
    for (keys, present) in [
        (Arc::new(NoKeys) as Arc<dyn KeySource>, false),
        (Arc::new(FakeKey), true),
    ] {
        let s = MockServer::start().await;
        mount(&s, sse(include_str!("fixtures/openai_basic.sse"))).await;
        collect(&client(&s, false, keys), Effort::Fast).await;
        let r = last_request(&s).await;
        assert_eq!(r.headers.get("authorization").is_some(), present);
    }
}

#[tokio::test]
async fn probe_lists_models() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":[{"id":"a"},{"id":"b"}]}"#),
        )
        .mount(&s)
        .await;
    let r = client(&s, false, Arc::new(NoKeys)).probe().await;
    assert!(r.reachable);
    assert_eq!(r.models, ["a", "b"]);
}

#[tokio::test]
async fn probe_body_over_cap_is_decode_error() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string("a".repeat((1 << 20) + 1)))
        .mount(&s)
        .await;
    let r = client(&s, false, Arc::new(NoKeys)).probe().await;
    assert!(!r.reachable);
    assert!(r.error.is_some_and(|e| e.contains("too large")));
}

#[tokio::test]
async fn oversize_sse_line_is_decode_error() {
    let s = MockServer::start().await;
    mount(
        &s,
        sse(&format!(
            "data: {}

",
            "a".repeat((1 << 20) + 1)
        )),
    )
    .await;
    let out = collect(&client(&s, false, Arc::new(NoKeys)), Effort::Fast).await;
    assert!(matches!(
        out.last(),
        Some(Err(ProviderError::Decode("sse line too long")))
    ));
}

#[tokio::test]
async fn oversize_sse_event_is_decode_error() {
    let s = MockServer::start().await;
    let body = format!(
        "data: {}
",
        "a".repeat(1000)
    )
    .repeat(1100);
    mount(&s, sse(&body)).await;
    let out = collect(&client(&s, false, Arc::new(NoKeys)), Effort::Fast).await;
    assert!(matches!(
        out.last(),
        Some(Err(ProviderError::Decode("sse event too large")))
    ));
}
