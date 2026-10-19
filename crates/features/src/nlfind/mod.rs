//! Natural-language finder (F06, ADR-005): a plain-language Find phrase is parsed offline, matched
//! against the file index, and opened only when the winner is clear and of an allowlisted type.
//! Otherwise optional decision engines may pick, else the ranked candidates are listed.
//! Engines see file names and parent folders only. Nothing here runs on the keystroke path except
//! the cheap `is_natural` check in `query`.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use crate::search::file_item;
use async_trait::async_trait;
use chrono::{Local, NaiveDate, TimeZone};
use decision::{
    Candidate, ChoiceQuestion, DateRange, DecisionEngine, KnownRoot, LlmPicker, Parsed,
    SystemOneEngine, is_natural, may_auto_open, parse,
};
use index_files::{FileHit, FindQuery};
use launcher_core::config::{Config, EngineEntry};
use launcher_core::store::{NlCacheEntry, Store};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use secrecy::SecretString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// Lowest fuzzy score the parser alone may auto-open.
// ponytail: fixed nucleo-scale constant; calibrate against real indexes if it misfires.
const AUTO_MIN_SCORE: u32 = 80;
/// The winner must beat the runner-up by this factor to auto-open.
const LEAD: f64 = 1.1;
/// Hits fetched per root.
const FETCH: usize = 20;
/// Default engine request timeout.
const ENGINE_TIMEOUT_MS: u64 = 3000;

/// What the finder needs from the desktop shell: the file index, opening a file, and engine keys.
/// Defined here so features never depend on the shell.
pub trait NlHost: Send + Sync {
    /// Runs a Find query over the index plus live overlay.
    fn find(&self, q: &FindQuery) -> Vec<FileHit>;
    /// Index root id for a known-folder label ("Downloads"), if that root is indexed.
    fn root_id(&self, label: &str) -> Option<u8>;
    /// Opens `path` with its default handler. Blocking.
    fn open(&self, path: &Path) -> Result<(), String>;
    /// Key of engine `id` bound to `host`, if stored. Blocking; never log it.
    fn engine_key(&self, id: &str, host: &str) -> Option<SecretString>;
}

/// The natural-language finder.
pub struct NlFind {
    host: Arc<dyn NlHost>,
    ctx: Mutex<Option<FeatureCtx>>,
}

impl NlFind {
    /// Finder over `host`. Registered by the shell, which owns the host.
    pub fn new(host: Arc<dyn NlHost>) -> Self {
        Self {
            host,
            ctx: Mutex::new(None),
        }
    }
}

fn root_label(r: KnownRoot) -> &'static str {
    match r {
        KnownRoot::Desktop => "Desktop",
        KnownRoot::Documents => "Documents",
        KnownRoot::Downloads => "Downloads",
        KnownRoot::Pictures => "Pictures",
        KnownRoot::Music => "Music",
        KnownRoot::Videos => "Videos",
    }
}

/// Local midnight at the start of `d` as Unix seconds (UTC midnight if local time is ambiguous).
fn day_start(d: NaiveDate) -> i64 {
    let t = d.and_hms_opt(0, 0, 0).unwrap_or_default();
    Local
        .from_local_datetime(&t)
        .earliest()
        .map_or_else(|| t.and_utc().timestamp(), |l| l.timestamp())
}

fn bounds(r: DateRange) -> (i64, i64) {
    (day_start(r.start), day_start(r.end) + 86_400 - 1)
}

fn cache_key(text: &str) -> String {
    text.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn ext_of(h: &FileHit) -> &str {
    if h.is_dir {
        return "";
    }
    Path::new(&h.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
}

/// Runs the parsed filters through the index. Roots that are not indexed contribute nothing.
fn search(host: &dyn NlHost, p: &Parsed) -> Vec<FileHit> {
    let (after, before) = p.modified.map_or((None, None), |r| {
        let (a, b) = bounds(r);
        (Some(a), Some(b))
    });
    let base = FindQuery {
        text: p.terms.join(" "),
        exts: p.exts.clone(),
        modified_after: after,
        modified_before: before,
        limit: FETCH,
        ..FindQuery::default()
    };
    let mut hits: Vec<FileHit> = if p.roots.is_empty() {
        host.find(&base)
    } else {
        p.roots
            .iter()
            .filter_map(|r| host.root_id(root_label(*r)))
            .flat_map(|id| {
                host.find(&FindQuery {
                    root: Some(id),
                    ..base.clone()
                })
            })
            .collect()
    };
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(b.modified.cmp(&a.modified)));
    hits.dedup_by(|a, b| a.path == b.path);
    hits
}

/// Stage 1: a clear, allowlisted winner.
fn clear_winner(hits: &[FileHit]) -> bool {
    let Some(top) = hits.first() else {
        return false;
    };
    let ahead = hits
        .get(1)
        .is_none_or(|s| f64::from(top.score) >= f64::from(s.score) * LEAD);
    !top.is_dir && top.score >= AUTO_MIN_SCORE && ahead && may_auto_open(ext_of(top))
}

fn open_outcome(host: &dyn NlHost, path: &Path) -> Result<Outcome, FeatureError> {
    host.open(path)
        .map(|()| Outcome::HideLauncher)
        .map_err(FeatureError::Other)
}

fn join_err(e: tokio::task::JoinError) -> FeatureError {
    FeatureError::Other(e.to_string())
}

/// The whole decision flow for one phrase. Blocking pieces run on the blocking pool.
async fn resolve(
    host: Arc<dyn NlHost>,
    store: &Store,
    text: &str,
    today: NaiveDate,
    engines: &[Box<dyn DecisionEngine>],
    threshold: f64,
    max_cands: usize,
) -> Result<Outcome, FeatureError> {
    let key = cache_key(text);
    if let Ok(Some(c)) = store.nl_cache_get(&key) {
        let path = PathBuf::from(&c.chosen_path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if may_auto_open(ext) && path.is_file() {
            store.nl_cache_put(c);
            return tokio::task::spawn_blocking(move || open_outcome(&*host, &path))
                .await
                .map_err(join_err)?;
        }
    }
    let parsed = parse(text, today);
    let h = host.clone();
    let hits = tokio::task::spawn_blocking(move || search(&*h, &parsed))
        .await
        .map_err(join_err)?;

    let mut chosen: Option<(usize, &'static str, Option<f64>)> =
        clear_winner(&hits).then_some((0, "parser", None));
    if chosen.is_none() && hits.len() > 1 {
        let cands: Vec<Candidate> = hits
            .iter()
            .take(max_cands.clamp(1, 8))
            .map(|h| Candidate {
                name: h.name.clone(),
                parent: h.parent.clone(),
            })
            .collect();
        let n = cands.len();
        let q = ChoiceQuestion {
            phrase: text.to_owned(),
            candidates: cands,
        };
        for e in engines {
            match e.choose(&q).await {
                Ok(a) => {
                    let p = f64::from(a.p);
                    if p >= threshold && a.index < n && may_auto_open(ext_of(&hits[a.index])) {
                        chosen = Some((a.index, a.engine, Some(p)));
                    }
                    break; // an answer, accepted or not, ends the chain
                }
                Err(err) => tracing::debug!(engine = e.id(), %err, "engine skipped"),
            }
        }
    }
    if let Some((i, engine, p)) = chosen {
        let path = hits[i].path.clone();
        store.nl_cache_put(NlCacheEntry {
            cache_key: key,
            phrase: text.to_owned(),
            chosen_path: path.to_string_lossy().into_owned(),
            engine: engine.to_owned(),
            p,
        });
        return tokio::task::spawn_blocking(move || open_outcome(&*host, &path))
            .await
            .map_err(join_err)?;
    }
    Ok(Outcome::ShowItems {
        items: hits.into_iter().map(file_item).collect(),
    })
}

/// Engines enabled, in config order, and usable by config (URL set, key present where required).
/// Blocking: reads the vault.
fn build_engines(
    host: &dyn NlHost,
    ctx: &FeatureCtx,
    cfg: &Config,
) -> Vec<Box<dyn DecisionEngine>> {
    let mut out: Vec<Box<dyn DecisionEngine>> = Vec::new();
    for id in &cfg.decision.order {
        let Some(e) = cfg.decision.engines.get(id).filter(|e| e.enabled) else {
            continue;
        };
        let timeout = Duration::from_millis(e.timeout_ms.unwrap_or(ENGINE_TIMEOUT_MS));
        if id == "llm" {
            if let Some(p) = ctx.providers.load().first() {
                out.push(Box::new(LlmPicker::new(p, timeout)));
            }
        } else if let Some(s) = system_one(host, id, e, timeout) {
            out.push(Box::new(s));
        }
    }
    out
}

fn system_one(
    host: &dyn NlHost,
    id: &str,
    e: &EngineEntry,
    timeout: Duration,
) -> Option<SystemOneEngine> {
    let id: &'static str = match id {
        "laya" => "laya",
        "kev" => "kev",
        "jev" => "jev",
        _ => return None,
    };
    let base_url = Url::parse(e.base_url.as_deref().filter(|u| !u.is_empty())?).ok()?;
    let key_host = e
        .key_host
        .clone()
        .or_else(|| base_url.host_str().map(str::to_owned));
    let bearer = key_host.as_deref().and_then(|h| host.engine_key(id, h));
    if id == "jev" && bearer.is_none() {
        return None;
    }
    Some(SystemOneEngine {
        id,
        base_url,
        key_host,
        bearer,
        timeout,
    })
}

#[async_trait]
impl Feature for NlFind {
    fn id(&self) -> &'static str {
        "nlfind"
    }
    fn title(&self) -> &'static str {
        "Natural-language finder"
    }
    fn version_tag(&self) -> &'static str {
        "v0.1.0"
    }
    fn description(&self) -> &'static str {
        "Find files by describing them. Opens only a clear winner of a safe type; engines are optional."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find]
    }
    fn settings_schema(&self) -> SettingsSchema {
        // Candidates and threshold live on the Decision engine page.
        SettingsSchema::default()
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        *self.ctx.lock() = Some(ctx);
        Ok(())
    }
    fn stop(&self) {
        self.ctx.lock().take();
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let text = input.text.trim();
        if input.mode != Mode::Find || !is_natural(text) {
            return Vec::new();
        }
        vec![Item {
            id: ItemId("nlfind:run".into()),
            kind: ItemKind::Command,
            title: format!("Find: {text}"),
            subtitle: "Plain-language search · Enter".into(),
            icon: None,
            score: 1.0,
            source: "nlfind".into(),
            actions: vec![ActionKind::Primary],
            payload: serde_json::json!({ "text": text }),
        }]
    }
    async fn execute(&self, item: &Item, _action: Action) -> Result<Outcome, FeatureError> {
        let ctx = self
            .ctx
            .lock()
            .clone()
            .ok_or_else(|| FeatureError::Other("natural-language finder is off".into()))?;
        let text = item
            .payload
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| FeatureError::Other("missing phrase".into()))?
            .to_owned();
        let cfg = ctx.config.load_full();
        let (h, c, cf) = (self.host.clone(), ctx.clone(), cfg.clone());
        let engines = tokio::task::spawn_blocking(move || build_engines(&*h, &c, &cf))
            .await
            .map_err(join_err)?;
        resolve(
            self.host.clone(),
            &ctx.store,
            &text,
            Local::now().date_naive(),
            &engines,
            cfg.decision.threshold,
            cfg.decision.candidates as usize,
        )
        .await
    }
}

#[cfg(test)]
mod tests;
