//! F10 Quick capture: `log 250 lunch`, `note ...`, `todo ...` go to Obsidian, a Markdown file,
//! Notion or a webhook (ADR-010). Parsing is local; the Query provider is asked only when a
//! `log` line has no amount or no label, and its reply only fills fields.
//!
//! `query()` reads config and parses text: one row per configured target, no I/O. Everything
//! else (files, vault, network) happens in `execute()` off the async threads. Text that looks
//! like a secret is blocked before it is written or sent (ADR-011 rule 8).

mod markdown;
mod notion;
mod obsidian;
pub mod parse;
mod webhook;

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::media::spotify::read_capped;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use chrono::{Local, NaiveDate};
use futures::StreamExt;
use launcher_core::brand::APP_DISPLAY_NAME;
use launcher_core::detect::looks_secret;
use launcher_core::{Action, ActionKind, Config, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use parse::Parsed;
use providers::secret::SecretVault;
use providers::{Delta, Effort, Message, Provider, Request, Role};
use reqwest::RequestBuilder;
use secrecy::SecretString;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Request timeout for Notion and webhooks (BSD External API table).
const TIMEOUT: Duration = Duration::from_secs(10);
/// Largest reply read from Notion or a webhook.
const MAX_BODY: usize = 1 << 20;
/// Longest wait honoured for a 429 `Retry-After`.
const MAX_RETRY_AFTER: u64 = 5;
/// How long the Query provider may take to fill fields.
const FILL_TIMEOUT: Duration = Duration::from_secs(8);
const BLOCKED: &str = "Blocked: looks like a secret";

/// Capture failures. Messages never carry tokens or webhook URLs.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// The target is not set up.
    #[error("{0}")]
    NotConfigured(&'static str),
    /// Transport failure or timeout.
    #[error("Could not reach the service")]
    Network,
    /// Unexpected HTTP status.
    #[error("The service returned status {0}")]
    Status(u16),
    /// The webhook URL's host is not the one its secret is bound to.
    #[error("The webhook URL's host does not match the saved host")]
    HostMismatch,
    /// Local file failure.
    #[error("Could not write the file ({0})")]
    Io(String),
    /// Anything else, already user-readable.
    #[error("{0}")]
    Other(String),
}

impl CaptureError {
    /// Stable code for the UI (BSD 4: `not_configured`, `network`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) | Self::HostMismatch => "not_configured",
            Self::Network => "network",
            _ => "capture",
        }
    }
}

impl From<CaptureError> for FeatureError {
    fn from(e: CaptureError) -> Self {
        FeatureError::Other(e.to_string())
    }
}

/// A finished HTTP exchange (status and capped body).
pub(crate) struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    retry_after: Option<u64>,
}

/// When to send a request again.
pub(crate) enum Retry<'a> {
    /// Once on 429, after `Retry-After` (at most 5 s).
    RateLimit,
    /// After each delay on network error or 5xx.
    Delays(&'a [Duration]),
}

async fn once(req: RequestBuilder) -> Result<Reply, CaptureError> {
    let resp = req.send().await.map_err(|_| CaptureError::Network)?;
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let body = read_capped(resp, MAX_BODY)
        .await
        .ok_or_else(|| CaptureError::Other("The reply was too large".into()))?;
    Ok(Reply {
        status,
        body,
        retry_after,
    })
}

/// Sends the request `make` builds, retrying per `retry`. Never follows up on 4xx.
pub(crate) async fn send(
    make: impl Fn() -> RequestBuilder,
    retry: Retry<'_>,
) -> Result<Reply, CaptureError> {
    let mut attempt = 0;
    loop {
        let r = once(make()).await;
        let wait = match (&retry, &r) {
            (Retry::RateLimit, Ok(x)) if x.status == 429 && attempt == 0 => Some(
                Duration::from_secs(x.retry_after.unwrap_or(1).min(MAX_RETRY_AFTER)),
            ),
            (Retry::Delays(d), Err(CaptureError::Network)) => d.get(attempt).copied(),
            (Retry::Delays(d), Ok(x)) if x.status >= 500 => d.get(attempt).copied(),
            _ => None,
        };
        let Some(wait) = wait else { return r };
        tokio::time::sleep(wait).await;
        attempt += 1;
    }
}

/// One place a capture can go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// `obsidian`, `markdown`, `notion` or `webhook.<name>`.
    pub id: String,
    /// Row title.
    pub title: String,
}

/// The configured, enabled targets, in a fixed order.
pub fn targets(cfg: &Config) -> Vec<Target> {
    let i = &cfg.integrations;
    let mut v = Vec::new();
    let mut add = |id: String, title: String| v.push(Target { id, title });
    if !i.obsidian.vault.trim().is_empty() {
        add("obsidian".into(), "Add to Obsidian daily note".into());
    }
    if let Some(name) = Path::new(i.markdown_file.trim())
        .file_name()
        .and_then(|n| n.to_str())
    {
        add("markdown".into(), format!("Append to {name}"));
    }
    if !i.notion.database_id.is_empty() {
        add("notion".into(), "Add to Notion".into());
    }
    for w in i
        .webhooks
        .iter()
        .filter(|w| w.enabled && !w.host.is_empty())
    {
        add(format!("webhook.{}", w.name), format!("Send to {}", w.name));
    }
    v
}

fn row(id: String, title: String, subtitle: String, payload: Value, actions: bool) -> Item {
    Item {
        id: ItemId(id),
        kind: ItemKind::Capture,
        title,
        subtitle,
        icon: None,
        score: 1.0,
        source: "capture".to_owned(),
        actions: if actions {
            vec![ActionKind::Primary]
        } else {
            vec![]
        },
        payload,
    }
}

fn join_err(e: tokio::task::JoinError) -> CaptureError {
    CaptureError::Other(format!("background task failed: {e}"))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CaptureError> {
    tokio::task::spawn_blocking(f).await.map_err(join_err)
}

/// Reads a vault secret off the async threads.
async fn secret(
    vault: &Arc<SecretVault>,
    id: String,
    host: String,
    missing: &'static str,
) -> Result<SecretString, CaptureError> {
    let v = Arc::clone(vault);
    blocking(move || v.integration_secret(&id, &host))
        .await?
        .ok_or(CaptureError::NotConfigured(missing))
}

fn webhook_of<'a>(
    cfg: &'a Config,
    id: &str,
) -> Result<&'a launcher_core::config::Webhook, CaptureError> {
    let name = id.strip_prefix("webhook.").unwrap_or_default();
    cfg.integrations
        .webhooks
        .iter()
        .find(|w| w.name == name && w.enabled)
        .ok_or(CaptureError::NotConfigured("That webhook is not enabled"))
}

async fn webhook_url(
    vault: &Arc<SecretVault>,
    cfg: &Config,
    id: &str,
) -> Result<(SecretString, String), CaptureError> {
    let w = webhook_of(cfg, id)?;
    let url = secret(
        vault,
        format!("webhook.{}", w.name),
        w.host.clone(),
        "Add the webhook URL first",
    )
    .await?;
    Ok((url, w.host.clone()))
}

/// Checks a target without sending data, except for a webhook, which gets a test POST (the
/// user clicked Test). Returns a message for the UI.
pub async fn test_target(
    vault: &Arc<SecretVault>,
    cfg: &Config,
    http: &reqwest::Client,
    id: &str,
) -> Result<String, CaptureError> {
    let i = &cfg.integrations;
    let today = Local::now().date_naive();
    match id {
        "obsidian" => {
            let o = i.obsidian.clone();
            blocking(move || obsidian::check(&o, today)).await??;
            Ok("The daily note folder is writable".into())
        }
        "markdown" => {
            let f = i.markdown_file.clone();
            blocking(move || markdown::check(&f)).await??;
            Ok("The file can be appended to".into())
        }
        "notion" => {
            if i.notion.database_id.is_empty() {
                return Err(CaptureError::NotConfigured("Set the Notion database ID"));
            }
            let token = secret(
                vault,
                "notion".into(),
                notion::HOST.into(),
                "Add the Notion token first",
            )
            .await?;
            let s = notion::load_schema(http, notion::BASE, &token, &i.notion.database_id).await?;
            Ok(format!(
                "Notion database found ({} properties)",
                s.property_count()
            ))
        }
        _ => {
            let (url, host) = webhook_url(vault, cfg, id).await?;
            let body = json!({
                "text": format!("Test from {APP_DISPLAY_NAME}"),
                "fields": {},
                "time": Local::now().to_rfc3339(),
            });
            webhook::post(http, &url, &host, &body, &webhook::DELAYS).await?;
            Ok("The webhook accepted a test message".into())
        }
    }
}

/// Fields a model may fill. Anything else in its reply rejects the whole reply.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    amount: Option<f64>,
    tags: Vec<String>,
    date: Option<String>,
}

/// Strict parse of the model reply (optional json fence, then exactly the three fields).
fn parse_fill(raw: &str) -> Option<Fill> {
    let mut s = raw.trim();
    if let Some(r) = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")) {
        s = r.strip_suffix("```")?.trim();
    }
    let f: Fill = serde_json::from_str(s).ok()?;
    let ok_amount = f
        .amount
        .is_none_or(|a| a.is_finite() && (0.0..1e12).contains(&a));
    let ok_tags = f.tags.len() <= 8
        && f.tags.iter().all(|t| {
            (1..=32).contains(&t.len())
                && t.chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        });
    let ok_date = f
        .date
        .as_deref()
        .is_none_or(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok());
    (ok_amount && ok_tags && ok_date).then_some(f)
}

/// Fills only the fields that are still empty.
fn apply_fill(p: &mut Parsed, f: Fill) {
    p.amount = p.amount.or(f.amount);
    if p.tags.is_empty() {
        p.tags = f.tags.into_iter().map(|t| t.to_lowercase()).collect();
    }
    p.date = p.date.or_else(|| {
        f.date
            .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
    });
}

async fn ask_provider(p: Arc<dyn Provider>, rest: &str) -> Option<String> {
    let prompt = format!(
        "Extract fields from this note. Reply with strict JSON only: \
         {{\"amount\": <number or null>, \"tags\": [<short words>], \"date\": <\"YYYY-MM-DD\" or null>}}.\nNote: {rest}"
    );
    let req = Request::new(
        vec![Message {
            role: Role::User,
            content: prompt,
        }],
        Effort::Fast,
    );
    let mut stream = p.stream(req, CancellationToken::new());
    let mut out = String::new();
    while let Some(d) = stream.next().await {
        match d.ok()? {
            Delta::Text(t) if out.len() + t.len() <= 4096 => out.push_str(&t),
            Delta::Text(_) => return None,
            Delta::Thinking => {}
            Delta::Done(_) => break,
        }
    }
    Some(out)
}

/// The Quick capture feature.
#[derive(Default)]
pub struct Capture {
    vault: Option<Arc<SecretVault>>,
    ctx: Mutex<Option<FeatureCtx>>,
    http: Mutex<Option<reqwest::Client>>,
    notion: tokio::sync::Mutex<Option<(String, notion::Schema)>>,
}

impl Capture {
    /// Capture with access to the secret vault (Notion token, webhook URLs).
    pub fn new(vault: Arc<SecretVault>) -> Self {
        Self {
            vault: Some(vault),
            ..Self::default()
        }
    }

    fn client(&self) -> Result<reqwest::Client, CaptureError> {
        let mut g = self.http.lock();
        if let Some(c) = &*g {
            return Ok(c.clone());
        }
        let c = providers::http::authed_client()
            .map_err(|_| CaptureError::Other("http client failed".into()))?;
        *g = Some(c.clone());
        Ok(c)
    }

    /// Asks the Query provider to fill empty fields, only when one is enabled. Failures mean
    /// "capture the raw text".
    async fn fill(&self, p: &mut Parsed, rest: &str, ctx: &FeatureCtx) {
        if !fill_allowed(&ctx.config.load()) {
            return;
        }
        let Some(provider) = ctx.providers.load().first() else {
            return;
        };
        let raw = tokio::time::timeout(FILL_TIMEOUT, ask_provider(provider, rest))
            .await
            .ok()
            .flatten();
        if let Some(f) = raw.as_deref().and_then(parse_fill) {
            apply_fill(p, f);
        }
    }

    async fn deliver(&self, cfg: &Config, id: &str, p: &Parsed) -> Result<(), CaptureError> {
        let now = Local::now();
        let (today, line) = (now.date_naive(), p.line(now.time()));
        let i = &cfg.integrations;
        match id {
            "obsidian" => {
                let (o, date) = (i.obsidian.clone(), p.date.unwrap_or(today));
                blocking(move || obsidian::append(&o, date, &line)).await?
            }
            "markdown" => {
                let f = i.markdown_file.clone();
                blocking(move || markdown::append(&f, &line)).await?
            }
            "notion" => self.to_notion(cfg, p, today).await,
            _ => {
                let vault = self
                    .vault
                    .as_ref()
                    .ok_or(CaptureError::NotConfigured("No secret store"))?;
                let (url, host) = webhook_url(vault, cfg, id).await?;
                let body =
                    json!({ "text": p.text, "fields": p.fields(), "time": now.to_rfc3339() });
                webhook::post(&self.client()?, &url, &host, &body, &webhook::DELAYS).await
            }
        }
    }

    async fn to_notion(
        &self,
        cfg: &Config,
        p: &Parsed,
        today: NaiveDate,
    ) -> Result<(), CaptureError> {
        let vault = self
            .vault
            .as_ref()
            .ok_or(CaptureError::NotConfigured("No secret store"))?;
        let db = cfg.integrations.notion.database_id.clone();
        let token = secret(
            vault,
            "notion".into(),
            notion::HOST.into(),
            "Add the Notion token first",
        )
        .await?;
        let http = self.client()?;
        let mut cache = self.notion.lock().await;
        let schema = match &*cache {
            Some((d, s)) if *d == db => s.clone(),
            _ => {
                let s = notion::load_schema(&http, notion::BASE, &token, &db).await?;
                *cache = Some((db, s.clone()));
                s
            }
        };
        drop(cache);
        notion::create_page(&http, notion::BASE, &token, &schema.page(p, today)?).await
    }
}

#[async_trait]
impl Feature for Capture {
    fn id(&self) -> &'static str {
        "capture"
    }
    fn title(&self) -> &'static str {
        "Quick capture"
    }
    fn version_tag(&self) -> &'static str {
        "v0.1.0"
    }
    fn description(&self) -> &'static str {
        "\"log 250 lunch\", \"note ...\", \"todo ...\" go to Obsidian, a Markdown file, Notion or a webhook."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema::default()
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        *self.ctx.lock() = Some(ctx);
        Ok(())
    }
    fn stop(&self) {
        *self.ctx.lock() = None;
        *self.http.lock() = None;
        if let Ok(mut n) = self.notion.try_lock() {
            *n = None;
        }
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let Some((kind, rest)) = parse::split_keyword(&input.text) else {
            return Vec::new();
        };
        if rest.is_empty() {
            return Vec::new();
        }
        let Some(cfg) = self.ctx.lock().as_ref().map(|c| c.config.load_full()) else {
            return Vec::new();
        };
        let raw = input.text.trim().to_owned();
        if looks_secret(rest).is_some() {
            let sub = "Not captured. Remove the key or token from the text.".to_owned();
            let payload = json!({ "blocked": true });
            return vec![row(
                "capture:blocked".into(),
                BLOCKED.into(),
                sub,
                payload,
                false,
            )];
        }
        let ts = targets(&cfg);
        if ts.is_empty() {
            let payload = json!({ "open_settings": "integrations" });
            let sub = "Pick Obsidian, a Markdown file, Notion or a webhook".to_owned();
            return vec![row(
                "capture:setup".into(),
                "Set up quick capture".into(),
                sub,
                payload,
                true,
            )];
        }
        let preview = parse::parse(kind, rest, Local::now().date_naive()).preview();
        ts.into_iter()
            .map(|t| {
                let payload = json!({ "target": t.id, "raw": raw });
                row(
                    format!("capture:{}", t.id),
                    t.title,
                    preview.clone(),
                    payload,
                    true,
                )
            })
            .collect()
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        let pl = &item.payload;
        if pl.get("open_settings").is_some() {
            return Ok(Outcome::Done);
        }
        let ctx = self
            .ctx
            .lock()
            .clone()
            .ok_or_else(|| FeatureError::Other("Quick capture is off".into()))?;
        let id = pl.get("target").and_then(Value::as_str).unwrap_or_default();
        let raw = pl.get("raw").and_then(Value::as_str).unwrap_or_default();
        let (kind, rest) = parse::split_keyword(raw)
            .ok_or_else(|| FeatureError::Other("Nothing to capture".into()))?;
        if pl.get("blocked").is_some() || looks_secret(rest).is_some() {
            return Err(FeatureError::SecretBlocked("capture"));
        }
        let mut p = parse::parse(kind, rest, Local::now().date_naive());
        if !p.is_clear() {
            self.fill(&mut p, rest, &ctx).await;
        }
        if p.text.is_empty() {
            p.text = rest.to_owned();
        }
        let cfg = ctx.config.load_full();
        if !targets(&cfg).iter().any(|t| t.id == id) {
            return Err(CaptureError::NotConfigured("That target is not set up").into());
        }
        self.deliver(&cfg, id, &p).await?;
        Ok(Outcome::HideLauncher)
    }
}

/// Text goes to a model provider only while Query mode is on.
fn fill_allowed(cfg: &Config) -> bool {
    cfg.modes.query.enabled
}

#[cfg(test)]
mod tests;
