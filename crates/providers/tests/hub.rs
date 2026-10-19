//! Hub fallback and cancellation.

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use providers::hub::{HubEvent, ProviderHub};
use providers::{
    Delta, Effort, Message, NoKeys, OpenAiCompat, ProbeResult, Provider, ProviderConfig,
    ProviderError, Request, Role, Usage, Wire,
};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cfg(id: &str, base: &str) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        wire: Wire::OpenAi,
        base_url: base.parse().unwrap(),
        model: "m".into(),
        key_host: None,
        supports_effort: false,
    }
}

fn http(id: &str, s: &MockServer) -> Arc<dyn Provider> {
    Arc::new(OpenAiCompat::new(cfg(id, &format!("{}/v1", s.uri())), Arc::new(NoKeys)).unwrap())
}

async fn server(resp: ResponseTemplate) -> MockServer {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(resp)
        .mount(&s)
        .await;
    s
}

fn ok_body() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_string(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n",
        )
}

fn req() -> Request {
    Request::new(
        vec![Message {
            role: Role::User,
            content: "q".into(),
        }],
        Effort::Fast,
    )
}

async fn collect(hub: &ProviderHub) -> Vec<HubEvent> {
    hub.ask(req(), CancellationToken::new()).collect().await
}

fn code(e: &HubEvent) -> &str {
    match e {
        HubEvent::Error { code, .. } => code,
        _ => "",
    }
}

#[tokio::test]
async fn fallback_on_503() {
    let a = server(ResponseTemplate::new(503)).await;
    let b = server(ok_body()).await;
    let ev = collect(&ProviderHub::new(vec![http("a", &a), http("b", &b)])).await;
    assert!(matches!(&ev[1], HubEvent::Fallback { from, to, .. } if from == "a" && to == "b"));
    assert!(ev.contains(&HubEvent::Delta { text: "hi".into() }));
    assert!(matches!(ev.last(), Some(HubEvent::Done { provider }) if provider == "b"));
}

#[tokio::test]
async fn no_fallback_on_401() {
    let a = server(ResponseTemplate::new(401).set_body_string("sk-secret-body")).await;
    let b = server(ok_body()).await;
    let ev = collect(&ProviderHub::new(vec![http("a", &a), http("b", &b)])).await;
    assert_eq!(ev.len(), 2);
    assert_eq!(code(&ev[1]), "provider_error");
    assert!(!format!("{ev:?}").contains("sk-secret"));
}

#[tokio::test]
async fn all_failed_and_empty() {
    let a = server(ResponseTemplate::new(500)).await;
    let ev = collect(&ProviderHub::new(vec![http("a", &a)])).await;
    assert_eq!(code(ev.last().unwrap()), "all_failed");
    let hub = ProviderHub::new(vec![]);
    assert!(hub.is_empty());
    assert_eq!(code(&collect(&hub).await[0]), "no_provider");
}

struct Fake {
    cfg: ProviderConfig,
    items: Vec<Result<Delta, ProviderError>>,
    hang: bool,
}

#[async_trait]
impl Provider for Fake {
    fn config(&self) -> &ProviderConfig {
        &self.cfg
    }
    fn stream(
        &self,
        _: Request,
        _: CancellationToken,
    ) -> BoxStream<'static, Result<Delta, ProviderError>> {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|r| match r {
                Ok(d) => Ok(d.clone()),
                Err(_) => Err(ProviderError::Status { code: 503 }),
            })
            .collect();
        let s = stream::iter(items);
        if self.hang {
            s.chain(stream::pending()).boxed()
        } else {
            s.boxed()
        }
    }
    async fn probe(&self) -> ProbeResult {
        unreachable!()
    }
}

fn fake(id: &str, items: Vec<Result<Delta, ProviderError>>, hang: bool) -> Arc<dyn Provider> {
    Arc::new(Fake {
        cfg: cfg(id, "http://localhost:1/v1"),
        items,
        hang,
    })
}

#[tokio::test]
async fn mid_stream_failure_no_fallback() {
    let a = fake(
        "a",
        vec![
            Ok(Delta::Text("x".into())),
            Err(ProviderError::Status { code: 503 }),
        ],
        false,
    );
    let b = fake("b", vec![Ok(Delta::Done(Usage::default()))], false);
    let ev = collect(&ProviderHub::new(vec![a, b])).await;
    assert!(!ev.iter().any(|e| matches!(e, HubEvent::Fallback { .. })));
    assert_eq!(code(ev.last().unwrap()), "stream_failed");
}

#[tokio::test]
async fn cancel_stops_silently() {
    let a = fake("a", vec![Ok(Delta::Text("x".into()))], true);
    let cancel = CancellationToken::new();
    let mut s = ProviderHub::new(vec![a]).ask(req(), cancel.clone());
    assert!(matches!(s.next().await, Some(HubEvent::Started { .. })));
    assert!(matches!(s.next().await, Some(HubEvent::Delta { .. })));
    cancel.cancel();
    let rest = tokio::time::timeout(Duration::from_secs(2), s.next()).await;
    assert!(matches!(rest, Ok(None)));
}

#[tokio::test]
async fn stalled_server_times_out_and_falls_back() {
    let a = server(ok_body().set_delay(Duration::from_secs(5))).await;
    let b = server(ok_body()).await;
    let hub = ProviderHub::new(vec![http("a", &a), http("b", &b)])
        .with_first_token_timeout(Duration::from_millis(300));
    let ev = collect(&hub).await;
    assert!(
        matches!(&ev[1], HubEvent::Fallback { from, reason, .. } if from == "a" && reason.contains("first token"))
    );
    assert!(matches!(ev.last(), Some(HubEvent::Done { provider }) if provider == "b"));
}

#[tokio::test]
async fn override_goes_first_with_its_model_and_replaces_same_id() {
    let done = || vec![Ok(Delta::Done(Usage::default()))];
    let a = fake("a", done(), false);
    let b = fake("b", vec![Err(ProviderError::Status { code: 503 })], false);
    let mut over = cfg("b", "http://localhost:1/v1");
    over.model = "big".into();
    let b2: Arc<dyn Provider> = Arc::new(Fake {
        cfg: over,
        items: vec![Err(ProviderError::Status { code: 503 })],
        hang: false,
    });
    let hub = ProviderHub::new(vec![a, b]);
    let ev: Vec<_> = hub
        .ask_with(req(), CancellationToken::new(), Some(b2))
        .collect()
        .await;
    assert_eq!(
        ev[0],
        HubEvent::Started {
            provider: "b".into(),
            model: "big".into()
        }
    );
    assert!(matches!(&ev[1], HubEvent::Fallback { from, to, .. } if from == "b" && to == "a"));
    assert!(matches!(ev.last(), Some(HubEvent::Done { provider }) if provider == "a"));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, HubEvent::Fallback { .. }))
            .count(),
        1
    );
}
