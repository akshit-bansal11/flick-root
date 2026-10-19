//! Webhook target: POST `{text, fields, time}` as JSON to a URL that is itself a secret.
//! The URL's host must equal the host the secret is bound to; redirects are never followed.

use super::{CaptureError, Retry, TIMEOUT, send};
use providers::http::check_url;
use reqwest::Method;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::time::Duration;
use url::Url;

/// Waits before the first and second retry (network error or 5xx).
pub const DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];

/// Posts `body` to `url` after checking it is https (or loopback http) on `bound_host`.
pub async fn post(
    http: &reqwest::Client,
    url: &SecretString,
    bound_host: &str,
    body: &Value,
    delays: &[Duration],
) -> Result<(), CaptureError> {
    let u = Url::parse(url.expose_secret())
        .map_err(|_| CaptureError::NotConfigured("The webhook URL is not a valid URL"))?;
    check_url(&u, bound_host).map_err(|_| CaptureError::HostMismatch)?;
    let bytes = serde_json::to_vec(body).map_err(|_| CaptureError::Other("bad body".into()))?;
    let r = send(
        || {
            http.request(Method::POST, u.clone())
                .timeout(TIMEOUT)
                .header("Content-Type", "application/json")
                .body(bytes.clone())
        },
        Retry::Delays(delays),
    )
    .await?;
    match r.status {
        200..=299 => Ok(()),
        s => Err(CaptureError::Status(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FAST: [Duration; 2] = [Duration::from_millis(5), Duration::from_millis(5)];

    fn url(s: &MockServer, p: &str) -> SecretString {
        SecretString::from(format!("{}{p}", s.uri()))
    }

    #[tokio::test]
    async fn sends_the_json_body() {
        let s = MockServer::start().await;
        let body = json!({ "text": "lunch", "fields": { "amount": 250 }, "time": "t" });
        Mock::given(method("POST"))
            .and(path("/hook/secret"))
            .and(body_json(body.clone()))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        post(&http, &url(&s, "/hook/secret"), "127.0.0.1", &body, &FAST)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn retries_5xx_twice_then_gives_up() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .expect(3)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        let r = post(&http, &url(&s, "/h"), "127.0.0.1", &json!({}), &FAST).await;
        assert_eq!(r, Err(CaptureError::Status(503)));
    }

    #[tokio::test]
    async fn a_5xx_then_success_is_ok_and_4xx_is_not_retried() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/flaky"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/flaky"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/gone"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        let b = json!({});
        post(&http, &url(&s, "/flaky"), "127.0.0.1", &b, &FAST)
            .await
            .unwrap();
        let r = post(&http, &url(&s, "/gone"), "127.0.0.1", &b, &FAST).await;
        assert_eq!(r, Err(CaptureError::Status(404)));
    }

    #[tokio::test]
    async fn host_mismatch_sends_nothing() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        let r = post(
            &http,
            &url(&s, "/h"),
            "hooks.example.com",
            &json!({}),
            &FAST,
        )
        .await;
        assert_eq!(r, Err(CaptureError::HostMismatch));
        let bad = SecretString::from("not a url".to_owned());
        let r = post(&http, &bad, "x", &json!({}), &FAST).await;
        assert!(matches!(r, Err(CaptureError::NotConfigured(_))));
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/elsewhere"))
            .expect(1)
            .mount(&s)
            .await;
        let http = providers::http::authed_client().unwrap();
        let r = post(&http, &url(&s, "/h"), "127.0.0.1", &json!({}), &FAST).await;
        assert_eq!(r, Err(CaptureError::Status(302)));
    }
}
