//! Local detection against fake servers.

use providers::detect::detect_targets;
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn finds_server_and_omits_dead_one() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":[{"id":"a"},{"id":"b"}]}"#),
        )
        .mount(&s)
        .await;
    let t = [
        ("ollama".to_owned(), format!("{}/v1", s.uri())),
        ("lmstudio".to_owned(), "http://127.0.0.1:1/v1".to_owned()),
    ];
    let d = detect_targets(&t, Duration::from_millis(500)).await;
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].preset_id, "ollama");
    assert_eq!(d[0].models, ["a", "b"]);
}

#[tokio::test]
async fn timeout_respected() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&s)
        .await;
    let t = [("ollama".to_owned(), format!("{}/v1", s.uri()))];
    let start = Instant::now();
    let d = detect_targets(&t, Duration::from_millis(200)).await;
    assert!(d.is_empty());
    assert!(start.elapsed() < Duration::from_millis(1000));
}
