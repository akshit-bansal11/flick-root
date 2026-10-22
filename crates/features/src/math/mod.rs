//! Inline math, units, time zones and ECB currency rates.
//!
//! Contract: the item's `payload.value` is the plain result text. On `CopyResult` the
//! shell copies `payload.value` to the clipboard; `execute` only answers `Outcome::Copied`.

mod calc;
mod rates;
mod zones;

/// Pays fend's one-off first-use cost; benches call it before timing. `Math::start` does it for the app.
pub use calc::warm;

use crate::ctx::{FeatureCtx, TaskScope};
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{Field, FieldKind, SettingsSchema};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use launcher_core::store::{RateRow, Store};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use rates::Rates;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Refetch when the cache is older than this (ECB publishes once per working day, about 16:00 CET).
const MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// Waits after a failed fetch: 1 min, 5 min, then a day.
const RETRY: [Duration; 3] = [Duration::from_secs(60), Duration::from_secs(300), MAX_AGE];
/// Total request timeout.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Rates responses are a few KB; anything bigger is not the ECB file.
const MAX_BODY: usize = 64 * 1024;

type Slot = Arc<ArcSwap<Option<Rates>>>;

/// Running state; dropped by `stop`.
struct Live {
    rates: Slot,
    tasks: TaskScope,
}

// ponytail: the registry builds `Math` as a unit value, so run state is process-wide; one registry per process.
static LIVE: Mutex<Option<Live>> = Mutex::new(None);

/// Inline math and units.
pub struct Math;

/// Answers `text` as a time-zone conversion first, then as math or units.
fn answer(text: &str, now: DateTime<Utc>) -> Option<String> {
    zones::convert(text, now).or_else(|| calc::evaluate(text))
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Cached rates, if any rows exist.
fn load_cache(store: &Store) -> Option<Rates> {
    let rows = store.rates_load().ok()?;
    // Only the latest date's rows: currencies the ECB dropped keep older rows that must not mix in.
    let latest = rows.iter().max_by(|a, b| a.rate_date.cmp(&b.rate_date))?;
    let mut per_eur: HashMap<String, f64> = rows
        .iter()
        .filter(|r| r.rate_date == latest.rate_date)
        .map(|r| (r.currency.clone(), r.per_eur))
        .collect();
    per_eur.insert("EUR".into(), 1.0);
    Some(Rates {
        date: latest.rate_date.clone(),
        fetched_at: latest.fetched_at,
        per_eur,
    })
}

/// One HTTPS request; errors are logged without the URL.
async fn fetch(http: &reqwest::Client) -> Result<Rates, String> {
    let resp = http
        .get(rates::URL)
        .timeout(TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("request failed: {}", e.without_url()))?;
    let body = resp
        .text()
        .await
        .map_err(|e| format!("read failed: {}", e.without_url()))?;
    if body.len() > MAX_BODY {
        return Err("response too large".into());
    }
    rates::parse(&body, now_ms()).map_err(|e| e.to_string())
}

/// Keeps rates fresh: cache first, then at most one fetch per `MAX_AGE`, never on the keystroke path.
async fn refresh_loop(ctx: FeatureCtx, slot: Slot, network: bool) {
    let token = ctx.tasks.token();
    let store = ctx.store.clone();
    let cached = tokio::task::spawn_blocking(move || load_cache(&store))
        .await
        .ok()
        .flatten();
    if let Some(r) = cached {
        slot.store(Arc::new(Some(r)));
    }
    if !network {
        return;
    }
    let max_ms = i64::try_from(MAX_AGE.as_millis()).unwrap_or(i64::MAX);
    let mut failures = 0usize;
    loop {
        let age_ms = slot
            .load()
            .as_ref()
            .as_ref()
            .map_or(i64::MAX, |r| now_ms().saturating_sub(r.fetched_at));
        let wait = if age_ms >= max_ms {
            let fetched = tokio::select! {
                () = token.cancelled() => return,
                r = fetch(&ctx.http) => r,
            };
            match fetched {
                Ok(r) => {
                    failures = 0;
                    let rows = r
                        .per_eur
                        .iter()
                        .map(|(c, v)| RateRow {
                            currency: c.clone(),
                            per_eur: *v,
                            rate_date: r.date.clone(),
                            fetched_at: r.fetched_at,
                        })
                        .collect();
                    ctx.store.rates_upsert(rows);
                    slot.store(Arc::new(Some(r)));
                    MAX_AGE
                }
                Err(e) => {
                    tracing::warn!(error = %e, "currency rates fetch failed, keeping the cache");
                    let d = RETRY[failures.min(RETRY.len() - 1)];
                    failures += 1;
                    d
                }
            }
        } else {
            Duration::from_millis(u64::try_from(max_ms - age_ms).unwrap_or(0))
        };
        tokio::select! {
            () = token.cancelled() => return,
            () = tokio::time::sleep(wait) => {}
        }
    }
}

fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn result_item(text: &str, value: String, subtitle: String) -> Item {
    Item {
        id: ItemId(format!("math:{text}")),
        kind: ItemKind::Result,
        title: value.clone(),
        subtitle,
        icon: None,
        score: 1000.0,
        source: "math".into(),
        actions: vec![ActionKind::CopyResult],
        payload: serde_json::json!({ "value": value }),
    }
}

/// The currency answer for `text`, when it is a currency question the rates can answer.
fn currency_item(text: &str, r: &Rates, now_ms: i64) -> Option<Item> {
    let ask = rates::parse_ask(text, r)?;
    let value = rates::format_amount(r.convert(ask.amount, &ask.from, &ask.to)?, &ask.to);
    Some(result_item(
        text,
        value,
        rates::subtitle(&norm(text), r, now_ms),
    ))
}

#[async_trait]
impl Feature for Math {
    fn id(&self) -> &'static str {
        "math"
    }
    fn title(&self) -> &'static str {
        "Inline math and units"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Results while you type after `=`: math, units, time zones and currency."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            fields: vec![Field {
                key: "currency_refresh".into(),
                label: "Update currency rates".into(),
                help: "Fetch ECB rates once a day. Off uses the last saved rates.".into(),
                kind: FieldKind::Bool,
                default: serde_json::json!(true),
                restart: true,
            }],
        }
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        let network = ctx
            .options("math")
            .get("currency_refresh")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        let slot: Slot = Arc::new(ArcSwap::from_pointee(None));
        // Warm fend off the keystroke path; queries return nothing until it finishes.
        ctx.tasks.spawn(async {
            let _ = tokio::task::spawn_blocking(calc::warm).await;
        });
        ctx.tasks
            .spawn(refresh_loop(ctx.clone(), slot.clone(), network));
        *LIVE.lock() = Some(Live {
            rates: slot,
            tasks: ctx.tasks,
        });
        Ok(())
    }
    fn stop(&self) {
        if let Some(l) = LIVE.lock().take() {
            l.tasks.token().cancel();
            l.rates.store(Arc::new(None));
        }
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let text = input.text.trim();
        // Cheap reject before touching the clock or fend.
        if !text.bytes().any(|b| b.is_ascii_digit()) && !text.to_lowercase().starts_with("now ") {
            return Vec::new();
        }
        let slot = LIVE.lock().as_ref().map(|l| l.rates.clone());
        if let Some(slot) = slot
            && let Some(r) = slot.load().as_ref()
            && let Some(item) = currency_item(text, r, now_ms())
        {
            return vec![item];
        }
        match answer(text, Utc::now()) {
            Some(value) => vec![result_item(text, value, norm(text))],
            None => Vec::new(),
        }
    }
    async fn execute(&self, _item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        match action.kind {
            ActionKind::CopyResult => Ok(Outcome::Copied),
            _ => Err(FeatureError::Other("unsupported action".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(t: &str) -> Input {
        Input {
            raw: t.into(),
            text: t.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    #[test]
    fn query_builds_one_result_item() {
        calc::warm();
        let items = Math.query(&input("2.5 kg to lb"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Result);
        assert!(items[0].title.starts_with("5.51"));
        assert_eq!(items[0].payload["value"], items[0].title.as_str());
        assert!(Math.query(&input("notepad")).is_empty());
        assert!(Math.query(&input("hello")).is_empty());
        assert!(Math.query(&input("2++")).is_empty());
    }

    #[tokio::test]
    async fn copy_result_answers_copied() {
        calc::warm();
        let item = &Math.query(&input("2+2*3"))[0];
        let a = Action {
            kind: ActionKind::CopyResult,
            confirm_token: None,
        };
        assert_eq!(Math.execute(item, a).await.ok(), Some(Outcome::Copied));
    }

    fn fixture() -> Rates {
        rates::parse(include_str!("../../tests/fixtures/eurofxref-daily.xml"), 0).unwrap()
    }

    #[test]
    fn currency_item_converts() {
        let r = fixture();
        let today = 5 * 86_400_000; // fixture fetched_at is 0
        let it = currency_item("250 usd in inr", &r, today).unwrap();
        assert_eq!(it.title, "23741.22 INR");
        assert!(it.subtitle.contains("rates from 5 days ago"));
        assert_eq!(it.actions, vec![ActionKind::CopyResult]);
        assert!(currency_item("hello", &r, today).is_none());
    }

    /// Start loads the saved rates without the network; stop drops them and leaves no task.
    #[tokio::test]
    async fn start_loads_cache_and_stop_drops_it() {
        use launcher_core::Config;
        let store = Arc::new(Store::open_in_memory().unwrap());
        store.rates_upsert(vec![
            RateRow {
                currency: "USD".into(),
                per_eur: 1.1253,
                rate_date: "2026-10-16".into(),
                fetched_at: now_ms(),
            },
            RateRow {
                currency: "INR".into(),
                per_eur: 106.864,
                rate_date: "2026-10-16".into(),
                fetched_at: now_ms(),
            },
        ]);
        store.flush();
        let mut cfg = Config::default();
        let mut opts = toml::Table::new();
        // Fresh cache plus refresh off: no network in this test.
        opts.insert("currency_refresh".into(), toml::Value::Boolean(false));
        cfg.features.insert("math".into(), opts);
        struct Sink;
        impl crate::ctx::EventSink for Sink {
            fn emit(&self, _: &str, _: serde_json::Value) {}
        }
        let ctx = FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(cfg)),
            store,
            providers: Arc::new(ArcSwap::from_pointee(providers::hub::ProviderHub::new(
                vec![],
            ))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        };
        Math.start(ctx.clone()).unwrap();
        let mut got = Vec::new();
        for _ in 0..100 {
            got = Math.query(&input("100 usd in inr"));
            if !got.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(got[0].title, "9496.49 INR");
        Math.stop();
        assert!(Math.query(&input("100 usd in inr")).is_empty());
        ctx.tasks.shutdown().await;
        assert_eq!(ctx.tasks.active(), 0);
    }
}
