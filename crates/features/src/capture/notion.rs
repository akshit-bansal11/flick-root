//! Notion target: an internal integration token plus a database id. The database is read
//! once per session to learn property names and types; parsed fields are mapped to them.
//!
//! Docs checked 2026-10-19 (developers.notion.com): `Notion-Version: 2026-03-11`; a database
//! holds data sources (`GET /v1/databases/{id}` lists them, `GET /v1/data_sources/{id}` has the
//! property schema); pages are created with `parent: {type: "data_source_id", ...}`.

use super::parse::Parsed;
use super::{CaptureError, Retry, TIMEOUT, send};
use chrono::NaiveDate;
use reqwest::Method;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

/// Host the integration token is bound to.
pub const HOST: &str = "api.notion.com";
/// Production base URL.
pub const BASE: &str = "https://api.notion.com";
/// Pinned API version.
pub const VERSION: &str = "2026-03-11";

/// Property names and types of the target data source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    ds_id: String,
    /// `(name, type)` in the order Notion returned them.
    props: Vec<(String, String)>,
}

fn request(
    http: &reqwest::Client,
    method: Method,
    url: String,
    token: &SecretString,
) -> reqwest::RequestBuilder {
    http.request(method, url)
        .bearer_auth(token.expose_secret())
        .header("Notion-Version", VERSION)
        .timeout(TIMEOUT)
}

fn status_err(status: u16) -> CaptureError {
    match status {
        401 | 403 => CaptureError::Other(
            "Notion refused the token - share the database with your integration".into(),
        ),
        404 => CaptureError::Other("Notion database not found - check the database ID".into()),
        400 => CaptureError::Other("Notion rejected the entry - check the property types".into()),
        s => CaptureError::Status(s),
    }
}

/// Reads the database once: first data source, then its properties.
pub async fn load_schema(
    http: &reqwest::Client,
    base: &str,
    token: &SecretString,
    db_id: &str,
) -> Result<Schema, CaptureError> {
    let db = send(
        || {
            request(
                http,
                Method::GET,
                format!("{base}/v1/databases/{db_id}"),
                token,
            )
        },
        Retry::RateLimit,
    )
    .await?;
    // Not a database (or no data_sources field): treat the id as a data source id.
    let ds_id = (db.status == 200)
        .then(|| serde_json::from_slice::<Value>(&db.body).ok())
        .flatten()
        .and_then(|v| v["data_sources"][0]["id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| db_id.to_owned());
    let ds = send(
        || {
            request(
                http,
                Method::GET,
                format!("{base}/v1/data_sources/{ds_id}"),
                token,
            )
        },
        Retry::RateLimit,
    )
    .await?;
    if ds.status != 200 {
        return Err(status_err(ds.status));
    }
    let v: Value = serde_json::from_slice(&ds.body)
        .map_err(|_| CaptureError::Other("Notion sent an unreadable reply".into()))?;
    let props = v["properties"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(name, p)| Some((name.clone(), p["type"].as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Ok(Schema { ds_id, props })
}

impl Schema {
    /// Number of properties read from the data source.
    pub fn property_count(&self) -> usize {
        self.props.len()
    }

    fn first(&self, kind: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(_, t)| t == kind)
            .map(|(n, _)| n.as_str())
    }

    /// The `POST /v1/pages` body: title <- text, number <- amount, date <- date (default
    /// today), multi_select <- tags. Properties the database lacks are skipped.
    pub fn page(&self, p: &Parsed, today: NaiveDate) -> Result<Value, CaptureError> {
        let title = self.first("title").ok_or(CaptureError::NotConfigured(
            "The Notion database has no title property",
        ))?;
        let mut props = serde_json::Map::new();
        props.insert(
            title.to_owned(),
            json!({ "title": [{ "text": { "content": p.text } }] }),
        );
        if let (Some(a), Some(n)) = (p.amount, self.first("number")) {
            props.insert(n.to_owned(), json!({ "number": a }));
        }
        if let Some(d) = self.first("date") {
            let day = p.date.unwrap_or(today).to_string();
            props.insert(d.to_owned(), json!({ "date": { "start": day } }));
        }
        if let (false, Some(m)) = (p.tags.is_empty(), self.first("multi_select")) {
            let tags: Vec<_> = p.tags.iter().map(|t| json!({ "name": t })).collect();
            props.insert(m.to_owned(), json!({ "multi_select": tags }));
        }
        Ok(json!({
            "parent": { "type": "data_source_id", "data_source_id": self.ds_id },
            "properties": props,
        }))
    }
}

/// Creates the page. One retry on 429, waiting `Retry-After` (at most 5 s).
pub async fn create_page(
    http: &reqwest::Client,
    base: &str,
    token: &SecretString,
    body: &Value,
) -> Result<(), CaptureError> {
    let bytes = serde_json::to_vec(body).map_err(|_| CaptureError::Other("bad body".into()))?;
    let r = send(
        || {
            request(http, Method::POST, format!("{base}/v1/pages"), token)
                .header("Content-Type", "application/json")
                .body(bytes.clone())
        },
        Retry::RateLimit,
    )
    .await?;
    match r.status {
        200..=299 => Ok(()),
        s => Err(status_err(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::parse::{Kind, parse};
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
    }

    fn token() -> SecretString {
        SecretString::from("secret_test_token".to_owned())
    }

    async fn server_with_schema() -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/databases/db1"))
            .and(header("Notion-Version", VERSION))
            .and(header("Authorization", "Bearer secret_test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "data_sources": [{ "id": "ds1", "name": "Money" }] })),
            )
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/data_sources/ds1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "properties": {
                    "Name": { "id": "t", "type": "title", "title": {} },
                    "Cost": { "id": "n", "type": "number", "number": {} },
                    "When": { "id": "d", "type": "date", "date": {} },
                    "Labels": { "id": "m", "type": "multi_select", "multi_select": {} },
                    "Done": { "id": "c", "type": "checkbox", "checkbox": {} },
                }})),
            )
            .mount(&s)
            .await;
        s
    }

    #[tokio::test]
    async fn maps_fields_by_type() {
        let s = server_with_schema().await;
        let http = reqwest::Client::new();
        let schema = load_schema(&http, &s.uri(), &token(), "db1").await.unwrap();
        assert_eq!(schema.props.len(), 5);
        let p = parse(Kind::Log, "250 lunch #food", day());
        let body = schema.page(&p, day()).unwrap();
        assert_eq!(body["parent"]["data_source_id"], "ds1");
        let pr = &body["properties"];
        assert_eq!(pr["Name"]["title"][0]["text"]["content"], "lunch");
        assert_eq!(pr["Cost"]["number"], 250.0);
        assert_eq!(pr["When"]["date"]["start"], "2026-10-19");
        assert_eq!(pr["Labels"]["multi_select"][0]["name"], "food");
        assert!(pr.get("Done").is_none());
    }

    #[tokio::test]
    async fn posts_the_page_and_retries_once_on_429() {
        let s = server_with_schema().await;
        Mock::given(method("POST"))
            .and(path("/v1/pages"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
            .up_to_n_times(1)
            .expect(1)
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/pages"))
            .and(body_partial_json(
                json!({ "parent": { "data_source_id": "ds1" } }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "p" })))
            .expect(1)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        let schema = load_schema(&http, &s.uri(), &token(), "db1").await.unwrap();
        let body = schema
            .page(&parse(Kind::Log, "5 tea", day()), day())
            .unwrap();
        create_page(&http, &s.uri(), &token(), &body).await.unwrap();
    }

    #[tokio::test]
    async fn second_429_is_an_error_and_bad_token_is_explained() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
            .expect(2)
            .mount(&s)
            .await;
        let http = reqwest::Client::new();
        let r = create_page(&http, &s.uri(), &token(), &json!({})).await;
        assert_eq!(r, Err(CaptureError::Status(429)));
        assert!(matches!(status_err(401), CaptureError::Other(m) if m.contains("share")));
    }

    #[test]
    fn no_title_property_is_not_configured() {
        let s = Schema {
            ds_id: "x".into(),
            props: vec![("Cost".into(), "number".into())],
        };
        let p = parse(Kind::Note, "hi", day());
        assert!(matches!(
            s.page(&p, day()),
            Err(CaptureError::NotConfigured(_))
        ));
    }
}
