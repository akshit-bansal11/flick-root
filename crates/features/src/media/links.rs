//! Search links for players, plus the optional YouTube Data API first-result lookup.

use crate::web::QUERY_SET;
use percent_encoding::utf8_percent_encode;
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;

fn enc(q: &str) -> String {
    utf8_percent_encode(q, QUERY_SET).to_string()
}

/// Spotify desktop search URI.
pub fn spotify(q: &str) -> String {
    format!("spotify:search:{}", enc(q))
}

/// YouTube results page for the default browser.
pub fn youtube(q: &str) -> String {
    format!("https://www.youtube.com/results?search_query={}", enc(q))
}

/// Base of the YouTube Data API (key bound to `www.googleapis.com`).
pub const YOUTUBE_API: &str = "https://www.googleapis.com";

/// First video for `q` via `search.list` (`maxResults=1`), as a watch URL. `None` on any
/// failure, so the caller falls back to the search page. The key never appears in errors.
pub async fn youtube_first(
    http: &reqwest::Client,
    base: &str,
    key: &SecretString,
    q: &str,
) -> Option<String> {
    let resp = http
        .get(format!("{base}/youtube/v3/search"))
        .timeout(Duration::from_secs(5))
        .query(&[
            ("part", "snippet"),
            ("type", "video"),
            ("maxResults", "1"),
            ("q", q),
            ("key", key.expose_secret()),
        ])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = super::spotify::read_capped(resp, 1 << 20).await?;
    let v: serde_json::Value = serde_json::from_slice(&body).ok()?;
    let id = v["items"][0]["id"]["videoId"].as_str()?;
    Some(format!("https://www.youtube.com/watch?v={id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn youtube_first_maps_and_falls_back() {
        let m = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/youtube/v3/search"))
            .and(query_param("maxResults", "1"))
            .and(query_param("key", "k"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{ "id": { "kind": "youtube#video", "videoId": "abc123" } }]
            })))
            .mount(&m)
            .await;
        let http = reqwest::Client::new();
        let key = SecretString::from("k".to_owned());
        assert_eq!(
            youtube_first(&http, &m.uri(), &key, "lofi")
                .await
                .as_deref(),
            Some("https://www.youtube.com/watch?v=abc123")
        );
        let bad = SecretString::from("nope".to_owned());
        assert_eq!(youtube_first(&http, &m.uri(), &bad, "lofi").await, None);
    }

    #[test]
    fn encodes_spaces_amp_unicode() {
        assert_eq!(spotify("a b"), "spotify:search:a%20b");
        assert_eq!(spotify("a&b=c"), "spotify:search:a%26b%3Dc");
        assert_eq!(
            youtube("café"),
            "https://www.youtube.com/results?search_query=caf%C3%A9"
        );
    }
}
