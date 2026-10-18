//! Frecency ranking (F01): a rank boost from how often and how recently an item was run, plus
//! per-prefix learning. Off means pure match order, because the registry stops calling the hooks.
//!
//! Formula: the store keeps one aggregate `score` per item as of `last_used_ms`, where each launch
//! adds 1 and older launches decay by `0.5^(age / half_life)`. Now the aggregate is
//! `s = score * 0.5^((now - last_used) / half_life)`, squashed into a bounded nudge
//! `FRECENCY_MAX * s / (s + SOFT)` (0..`FRECENCY_MAX`). A remembered prefix choice adds
//! `PREFIX_BOOST`, which beats any frecency nudge.

use crate::ctx::{FeatureCtx, TaskScope};
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{Field, FieldKind, SettingsSchema};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use launcher_core::store::{ClearScope, Store};
use launcher_core::{Action, Input, Item, Mode, Outcome};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;

/// Largest frecency boost (added to a 0..1 match score).
pub const FRECENCY_MAX: f32 = 0.6;
/// Aggregate score at which the boost reaches half of `FRECENCY_MAX`.
const SOFT: f64 = 3.0;
/// Boost for the remembered choice of the typed prefix.
pub const PREFIX_BOOST: f32 = 2.0;
const MS_PER_DAY: f64 = 86_400_000.0;

/// Decayed usage aggregate as of `last_used_ms`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// Aggregate as of `last_used_ms`.
    pub score: f64,
    /// Last launch, ms since epoch.
    pub last_used_ms: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

/// `score` decayed from `last` to `now` with the given half-life.
pub fn decayed(score: f64, last: i64, now: i64, half_life_days: f64) -> f64 {
    let age = (now - last).max(0) as f64 / MS_PER_DAY;
    score * 0.5f64.powf(age / half_life_days.max(1.0))
}

/// Bounded boost for decayed aggregate `s`.
fn squash(s: f64) -> f32 {
    FRECENCY_MAX * (s / (s + SOFT)) as f32
}

type Scores = HashMap<String, Score>;
type Prefixes = HashMap<String, Option<String>>;

struct Inner {
    store: Arc<Store>,
    handle: Handle,
    tasks: TaskScope,
    half_life: f64,
    learning: bool,
    scores: ArcSwap<Scores>,
    /// Lowercased prefix to remembered item key. `None` means looked up, nothing learned.
    prefixes: ArcSwap<Prefixes>,
}

impl Inner {
    fn boost(&self, input: &Input, item: &Item, now: i64) -> f32 {
        let mut b = 0.0;
        if let Some(s) = self.scores.load().get(&item.id.0) {
            b += squash(decayed(s.score, s.last_used_ms, now, self.half_life));
        }
        if self.learning && !input.text.is_empty() {
            let p: Cow<str> = if input.text.chars().any(char::is_uppercase) {
                Cow::Owned(input.text.to_lowercase())
            } else {
                Cow::Borrowed(&input.text)
            };
            if self
                .prefixes
                .load()
                .get(p.as_ref())
                .is_some_and(|k| k.as_deref() == Some(item.id.0.as_str()))
            {
                b += PREFIX_BOOST;
            }
        }
        b
    }

    fn chosen(&self, input: &Input, item: &Item, now: i64) {
        let key = item.id.0.clone();
        self.scores.rcu(|m| {
            let mut m = Scores::clone(m);
            let prev = m.get(&key).map_or(0.0, |s| {
                decayed(s.score, s.last_used_ms, now, self.half_life)
            });
            m.insert(
                key.clone(),
                Score {
                    score: prev + 1.0,
                    last_used_ms: now,
                },
            );
            m
        });
        let prefix =
            (self.learning && !input.text.trim().is_empty()).then(|| input.text.to_lowercase());
        if let Some(p) = &prefix {
            self.prefixes.rcu(|m| {
                let mut m = Prefixes::clone(m);
                m.insert(p.clone(), Some(key.clone()));
                m
            });
        }
        let kind = format!("{:?}", item.kind).to_lowercase();
        self.store
            .record_launch(&key, &kind, prefix.as_deref(), now);
        if let Some(p) = &prefix {
            self.store.record_prefix_choice(p, &key, now);
        }
    }

    /// Starts a one-off store lookup of the learned choice for `prefix` unless one is cached.
    fn ensure_prefix(self: &Arc<Self>, prefix: String) {
        if self.prefixes.load().contains_key(&prefix) {
            return;
        }
        self.prefixes.rcu(|m| {
            let mut m = Prefixes::clone(m);
            m.entry(prefix.clone()).or_insert(None);
            m
        });
        let me = self.clone();
        let _g = self.handle.enter();
        self.tasks.spawn(async move {
            let (st, p) = (me.store.clone(), prefix.clone());
            if let Ok(Ok(Some(k))) = tokio::task::spawn_blocking(move || st.prefix_choice(&p)).await
            {
                me.prefixes.rcu(|m| {
                    let mut m = Prefixes::clone(m);
                    m.insert(prefix.clone(), Some(k.clone()));
                    m
                });
            }
        });
    }
}

/// Frecency ranking.
#[derive(Default)]
pub struct Frecency {
    inner: ArcSwap<Option<Arc<Inner>>>,
}

impl Frecency {
    /// Clears stored history and the in-memory maps. Backs the `clear_history` button.
    pub fn clear_history(&self) {
        if let Some(i) = self.inner.load().as_ref() {
            i.store.clear(ClearScope::Frecency);
            i.scores.store(Arc::default());
            i.prefixes.store(Arc::default());
        }
    }
}

#[async_trait]
impl Feature for Frecency {
    fn id(&self) -> &'static str {
        "frecency"
    }
    fn title(&self) -> &'static str {
        "Frecency ranking"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Ranks by how often and how recently you used something, and learns per typed prefix."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command, Mode::Find]
    }
    /// `clear_history` is an action field: a Bool the Settings page renders as a button. It is
    /// never persisted; the page calls [`Frecency::clear_history`] instead of writing config.
    fn settings_schema(&self) -> SettingsSchema {
        let f = |key: &str, label: &str, help: &str, kind, default, restart| Field {
            key: key.into(),
            label: label.into(),
            help: help.into(),
            kind,
            default,
            restart,
        };
        SettingsSchema {
            fields: vec![
                f(
                    "half_life_days",
                    "Half-life (days)",
                    "How fast old launches stop counting.",
                    FieldKind::Number {
                        min: 1.0,
                        max: 90.0,
                        step: 1.0,
                    },
                    serde_json::json!(7),
                    true,
                ),
                f(
                    "learning",
                    "Learn from typed prefixes",
                    "Remember which result you pick for what you type.",
                    FieldKind::Bool,
                    serde_json::json!(true),
                    true,
                ),
                f(
                    "clear_history",
                    "Clear history",
                    "Action button: deletes all usage history and learned prefixes.",
                    FieldKind::Bool,
                    serde_json::json!(false),
                    false,
                ),
            ],
        }
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        let o = ctx.options("frecency");
        let days = o
            .get("half_life_days")
            .and_then(|v| v.as_integer().or_else(|| v.as_float().map(|x| x as i64)))
            .unwrap_or(7)
            .clamp(1, 90);
        let learning = o
            .get("learning")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        ctx.store
            .set_half_life_days(u32::try_from(days).unwrap_or(7));
        let handle = Handle::try_current().map_err(|e| FeatureError::Start {
            id: "frecency",
            reason: e.to_string(),
        })?;
        let inner = Arc::new(Inner {
            store: ctx.store.clone(),
            handle,
            tasks: ctx.tasks.clone(),
            half_life: days as f64,
            learning,
            scores: ArcSwap::default(),
            prefixes: ArcSwap::default(),
        });
        let (me, st) = (inner.clone(), ctx.store);
        ctx.tasks.spawn(async move {
            match tokio::task::spawn_blocking(move || st.load_frecency()).await {
                Ok(Ok(rows)) => drop(me.scores.rcu(|cur| {
                    // Launches recorded while loading win over older rows.
                    let mut m: Scores = rows
                        .iter()
                        .map(|r| {
                            (
                                r.item_key.clone(),
                                Score {
                                    score: r.score,
                                    last_used_ms: r.last_used_ms,
                                },
                            )
                        })
                        .collect();
                    m.extend(cur.iter().map(|(k, v)| (k.clone(), *v)));
                    m
                })),
                Ok(Err(e)) => tracing::warn!(%e, "frecency load failed"),
                Err(e) => tracing::warn!(%e, "frecency load task failed"),
            }
        });
        self.inner.store(Arc::new(Some(inner)));
        Ok(())
    }
    fn stop(&self) {
        self.inner.store(Arc::new(None));
    }
    /// Never returns items; its only job is to warm the prefix cache for this keystroke's text.
    fn query(&self, input: &Input) -> Vec<Item> {
        if let Some(i) = self.inner.load().as_ref()
            && i.learning
            && !input.text.is_empty()
        {
            i.ensure_prefix(input.text.to_lowercase());
        }
        Vec::new()
    }
    async fn execute(&self, _item: &Item, _action: Action) -> Result<Outcome, FeatureError> {
        Err(FeatureError::Other("frecency has no items".into()))
    }
    fn rank_boost(&self, input: &Input, item: &Item) -> f32 {
        let g = self.inner.load();
        (**g)
            .as_ref()
            .map_or(0.0, |i| i.boost(input, item, now_ms()))
    }
    fn on_chosen(&self, input: &Input, item: &Item) {
        if let Some(i) = self.inner.load().as_ref() {
            i.chosen(input, item, now_ms());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::EventSink;
    use crate::registry::Registry;
    use launcher_core::{Config, ItemId, ItemKind};
    use providers::hub::ProviderHub;
    use std::time::Duration;

    struct Sink;
    impl EventSink for Sink {
        fn emit(&self, _: &str, _: serde_json::Value) {}
    }

    fn ctx(learning: bool) -> FeatureCtx {
        let mut cfg = Config::default();
        let mut t = toml::Table::new();
        t.insert("learning".into(), toml::Value::Boolean(learning));
        cfg.features.insert("frecency".into(), t);
        FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(cfg)),
            store: Arc::new(Store::open_in_memory().unwrap()),
            providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        }
    }

    fn item(id: &str) -> Item {
        Item {
            id: ItemId(id.into()),
            kind: ItemKind::App,
            title: id.into(),
            subtitle: String::new(),
            icon: None,
            score: 1.0,
            source: "apps".into(),
            actions: vec![],
            payload: serde_json::Value::Null,
        }
    }

    fn input(text: &str) -> Input {
        Input {
            raw: text.into(),
            text: text.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }

    fn started(learning: bool) -> (Frecency, FeatureCtx) {
        let f = Frecency::default();
        let c = ctx(learning);
        f.start(c.clone()).unwrap();
        (f, c)
    }

    #[test]
    fn decay_halves_at_one_half_life() {
        let day = MS_PER_DAY as i64;
        assert!((decayed(4.0, 0, 7 * day, 7.0) - 2.0).abs() < 1e-9);
        assert!((decayed(4.0, 0, 14 * day, 7.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recent_beats_frequent_but_old() {
        let day = MS_PER_DAY as i64;
        let now = 100 * day;
        let old = decayed(20.0, 0, now, 7.0); // 20 launches, 100 days ago
        let recent = decayed(1.0, now - day, now, 7.0);
        assert!(squash(recent) > squash(old));
        assert!(squash(1e9) < FRECENCY_MAX + 1e-6);
    }

    #[tokio::test]
    async fn prefix_learning_picks_chosen_item() {
        let (f, _c) = started(true);
        f.on_chosen(&input("Ed"), &item("b"));
        let (a, b) = (item("a"), item("b"));
        assert!(f.rank_boost(&input("ed"), &b) >= PREFIX_BOOST);
        assert!(f.rank_boost(&input("ed"), &a) < 1e-6);
        assert!(f.rank_boost(&input("other"), &b) < PREFIX_BOOST);
        // Empty input: frecency alone.
        let e = f.rank_boost(&input(""), &b);
        assert!(e > 0.0 && e < FRECENCY_MAX);
    }

    #[tokio::test]
    async fn learning_off_ignores_prefix_map() {
        let (f, _c) = started(false);
        f.on_chosen(&input("ed"), &item("b"));
        let e = f.rank_boost(&input("ed"), &item("b"));
        assert!(e > 0.0 && e < PREFIX_BOOST);
    }

    #[tokio::test]
    async fn loads_history_and_clear_empties() {
        let c = ctx(true);
        c.store.record_launch("x", "app", None, now_ms());
        c.store.flush();
        let f = Frecency::default();
        f.start(c.clone()).unwrap();
        for _ in 0..100 {
            if f.rank_boost(&input(""), &item("x")) > 0.0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(f.rank_boost(&input(""), &item("x")) > 0.0);
        f.clear_history();
        assert_eq!(f.rank_boost(&input(""), &item("x")), 0.0);
        assert!(c.store.load_frecency().unwrap().is_empty());
    }

    #[tokio::test]
    async fn feature_off_keeps_match_order() {
        let c = ctx(true);
        let r = Registry::new();
        r.register(Arc::new(Frecency::default())).await;
        r.apply_config(&c.config.load(), &c).await;
        let (a, b) = (item("a"), item("b"));
        r.on_chosen_all(&input("a"), &b);
        assert!(r.rank_boost_all(&input("a"), &b) > 0.0);
        r.set_enabled("frecency", false, &c).await.unwrap();
        assert_eq!(r.rank_boost_all(&input("a"), &b), 0.0);
        assert_eq!(r.rank_boost_all(&input("a"), &a), 0.0);
    }
}
