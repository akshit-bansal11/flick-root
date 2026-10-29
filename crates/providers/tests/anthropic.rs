//! Wiremock tests for the Anthropic client. Keys here are fake.

use futures::StreamExt;
use providers::anthropic::{ANTHROPIC_VERSION, Anthropic};
use providers::{
    Delta, KeySource, Message, NoKeys, Provider, ProviderConfig, ProviderError, Request, Role, Wire,
};
use secrecy::SecretString;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct FakeKey;
impl KeySource for FakeKey {
    fn key_for(&self, _: &ProviderConfig) -> Result<Option<SecretString>, ProviderError> {
        Ok(Some(SecretString::from("sk-ant-fake-0000".to_owned())))
    }
}

fn client(server: &MockServer, keys: Arc<dyn KeySource>) -> Anthropic {
    let cfg = ProviderConfig {
        id: "a".into(),
        wire: Wire::Anthropic,
        base_url: server.uri().parse().unwrap(),
        model: "m".into(),
        key_host: None,
        supports_effort: true,
    };
    Anthropic::new(cfg, keys).unwrap()
}

fn req(effort: Option<&str>) -> Request {
    Request::new(
        vec![
            Message {
                role: Role::User,
                content: "hi".into(),
            },
            Message {
                role: Role::Assistant,
                content: "hello".into(),
            },
            Message {
                role: Role::User,
                content: "again".into(),
            },
        ],
        effort.map(str::to_owned),
    )
}

fn sse(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_string(body)
}

async fn mount(server: &MockServer, t: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(t)
        .mount(server)
        .await;
}

async fn collect(p: &Anthropic, e: Option<&str>) -> Vec<Result<Delta, ProviderError>> {
    p.stream(req(e), CancellationToken::new()).collect().await
}

async fn last_request(s: &MockServer) -> wiremock::Request {
    s.received_requests().await.unwrap().pop().unwrap()
}

#[tokio::test]
async fn deltas_in_order() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/anthropic_basic.sse"))).await;
    let got: Vec<_> = collect(&client(&s, Arc::new(NoKeys)), None)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    let texts: Vec<_> = got
        .iter()
        .filter_map(|d| match d {
            Delta::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Hel", "lo", "!"]);
    let Some(Delta::Done(u)) = got.last() else {
        panic!("no Done")
    };
    assert_eq!((u.prompt_tokens, u.completion_tokens), (7, 3));
}

#[tokio::test]
async fn thinking_block_yields_one_thinking() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/anthropic_thinking.sse"))).await;
    let got: Vec<_> = collect(&client(&s, Arc::new(NoKeys)), Some("high"))
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(got.iter().filter(|d| **d == Delta::Thinking).count(), 1);
    assert_eq!(got[0], Delta::Thinking);
    assert_eq!(got[1], Delta::Text("Answer".into()));
}

#[tokio::test]
async fn request_body_rules() {
    for effort in [None, Some("low"), Some("max")] {
        let s = MockServer::start().await;
        mount(&s, sse(include_str!("fixtures/anthropic_basic.sse"))).await;
        collect(&client(&s, Arc::new(NoKeys)), effort).await;
        let body: serde_json::Value = serde_json::from_slice(&last_request(&s).await.body).unwrap();
        for k in ["temperature", "top_p", "top_k"] {
            assert!(body.get(k).is_none(), "{k}");
        }
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 1024);
        assert!(body["system"].is_string());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.last().unwrap()["role"], "user");
        assert!(msgs.iter().all(|m| m["role"] != "system"));
        assert!(body.get("thinking").is_none());
        assert_eq!(body["output_config"]["effort"].as_str(), effort);
    }
}

#[tokio::test]
async fn assistant_last_is_rejected_without_request() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/anthropic_basic.sse"))).await;
    let r = Request::new(
        vec![Message {
            role: Role::Assistant,
            content: "prefill".into(),
        }],
        None,
    );
    let out: Vec<_> = client(&s, Arc::new(NoKeys))
        .stream(r, CancellationToken::new())
        .collect()
        .await;
    assert!(matches!(out[0], Err(ProviderError::Decode(_))));
    assert!(s.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn headers_and_key_only_when_present() {
    for (keys, present) in [
        (Arc::new(NoKeys) as Arc<dyn KeySource>, false),
        (Arc::new(FakeKey), true),
    ] {
        let s = MockServer::start().await;
        mount(&s, sse(include_str!("fixtures/anthropic_basic.sse"))).await;
        collect(&client(&s, keys), None).await;
        let r = last_request(&s).await;
        assert_eq!(r.headers.get("x-api-key").is_some(), present);
        assert_eq!(
            r.headers
                .get("anthropic-version")
                .unwrap()
                .to_str()
                .unwrap(),
            ANTHROPIC_VERSION
        );
        assert!(r.headers.get("authorization").is_none());
    }
}

#[tokio::test]
async fn error_event_is_error() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/anthropic_error.sse"))).await;
    let out = collect(&client(&s, Arc::new(NoKeys)), None).await;
    let Some(Err(e)) = out.into_iter().last() else {
        panic!("expected error")
    };
    assert!(matches!(e, ProviderError::Status { code: 529 }));
    assert!(e.is_fallback_worthy());
}

#[tokio::test]
async fn status_errors() {
    for (code, fallback) in [(529, true), (503, true), (401, false)] {
        let s = MockServer::start().await;
        mount(
            &s,
            ResponseTemplate::new(code).set_body_string("secret body"),
        )
        .await;
        let got = collect(&client(&s, Arc::new(NoKeys)), None).await;
        let Some(Err(e)) = got.into_iter().next() else {
            panic!("expected error")
        };
        assert!(matches!(e, ProviderError::Status { code: c } if c == code));
        assert_eq!(e.is_fallback_worthy(), fallback, "{code}");
    }
}

#[tokio::test]
async fn cancel_after_first_delta() {
    let s = MockServer::start().await;
    mount(&s, sse(include_str!("fixtures/anthropic_basic.sse"))).await;
    let token = CancellationToken::new();
    let mut st = client(&s, Arc::new(NoKeys)).stream(req(None), token.clone());
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
async fn probe_needs_key_then_lists_models() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":[{"id":"a"},{"id":"b"}]}"#),
        )
        .mount(&s)
        .await;
    let r = client(&s, Arc::new(NoKeys)).probe().await;
    assert!(!r.reachable);
    assert!(r.error.is_some_and(|e| e.contains("no key")));
    assert!(s.received_requests().await.unwrap().is_empty());
    let r = client(&s, Arc::new(FakeKey)).probe().await;
    assert!(r.reachable);
    assert_eq!(r.models, ["a", "b"]);
}

#[tokio::test]
async fn fetch_models_sends_key_and_version_and_parses() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "1000"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"id":"a","lifecycle":"active","capabilities":{"effort":
            {"supported":true,"low":{"supported":true},"medium":{"supported":true},
            "high":{"supported":true},"xhigh":null,"max":{"supported":false}}}},
            {"id":"r","lifecycle":"retired","capabilities":null}],
            "has_more":false,"first_id":"a","last_id":"r"}"#,
        ))
        .mount(&s)
        .await;
    let got = client(&s, Arc::new(FakeKey)).fetch_models().await.unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "a");
    assert_eq!(got[0].efforts, ["low", "medium", "high"]);
    let r = last_request(&s).await;
    assert!(r.headers.get("x-api-key").is_some());
    assert_eq!(
        r.headers
            .get("anthropic-version")
            .unwrap()
            .to_str()
            .unwrap(),
        ANTHROPIC_VERSION
    );
}

#[tokio::test]
async fn fetch_models_needs_key_and_hides_key_and_body() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_string("secret body"))
        .mount(&s)
        .await;
    let e = client(&s, Arc::new(NoKeys))
        .fetch_models()
        .await
        .unwrap_err();
    assert!(matches!(e, ProviderError::NoKey { .. }));
    assert!(s.received_requests().await.unwrap().is_empty());
    let e = client(&s, Arc::new(FakeKey))
        .fetch_models()
        .await
        .unwrap_err();
    assert!(matches!(e, ProviderError::Status { code: 401 }));
    let msg = e.to_string();
    assert!(!msg.contains("sk-ant-fake") && !msg.contains("secret body"));
}
