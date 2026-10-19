//! Decision-engine list, probe and configuration commands (BSD section 4, ADR-005).
use super::blocking;
use crate::error::CommandError;
use crate::state::AppState;
use decision::{DecisionEngine, SystemOneEngine};
use launcher_core::config::{Config, EngineEntry};
use parking_lot::Mutex;
use providers::http;
use providers::secret::{SecretName, SecretVault};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;
use url::Url;

const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Known engine ids and titles, in default order.
const ENGINES: [(&str, &str); 5] = [
    ("laya", "Laya"),
    ("kev", "Kev"),
    ("llm", "Your AI provider"),
    ("jev", "Jev"),
    ("custom", "Custom"),
];

/// One engine as the settings UI sees it. Carries no secret value.
#[derive(Debug, Serialize)]
pub struct EngineView {
    /// Engine id.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Enabled by the user.
    pub enabled: bool,
    /// Base URL; empty for `llm` and unset engines.
    pub base_url: String,
    /// Host a key is bound to (configured, else the base URL host).
    pub key_host: Option<String>,
    /// Whether the engine cannot be used without a key.
    pub needs_key: bool,
    /// Whether a key is stored for `key_host`.
    pub key_present: bool,
    /// Global candidate cap.
    pub candidates: u32,
    /// Global acceptance threshold.
    pub threshold: f64,
    /// Request timeout, if configured.
    pub timeout_ms: Option<u64>,
    /// Position in the engine order.
    pub order: usize,
}

/// Probe outcome for one engine.
#[derive(Debug, Clone, Serialize)]
pub struct EngineProbe {
    /// Engine id.
    pub id: String,
    /// `reachable`, `not_running`, `no_key` or `no_provider`.
    pub status: &'static str,
    /// Failure detail, if any.
    pub detail: Option<String>,
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_owned)
}

/// Host a key is bound to: the configured `key_host`, else the base URL host.
pub(crate) fn effective_host(e: &EngineEntry) -> Option<String> {
    e.key_host
        .clone()
        .or_else(|| e.base_url.as_deref().and_then(host_of))
}

fn key_present(vault: &SecretVault, id: &str, e: &EngineEntry) -> bool {
    effective_host(e)
        .and_then(|h| SecretName::parse(&format!("engine.{id}@{h}")).ok())
        .and_then(|n| vault.info(&n).ok())
        .is_some_and(|i| i.present)
}

fn views(vault: &SecretVault, cfg: &Config) -> Vec<EngineView> {
    let d = &cfg.decision;
    let mut ids: Vec<&str> = d.order.iter().map(String::as_str).collect();
    for (id, _) in ENGINES {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.iter()
        .enumerate()
        .filter_map(|(order, id)| {
            let title = ENGINES.iter().find(|(i, _)| i == id)?.1;
            let e = d.engines.get(*id).cloned().unwrap_or_default();
            Some(EngineView {
                id: (*id).to_owned(),
                title: title.to_owned(),
                enabled: e.enabled,
                base_url: e.base_url.clone().unwrap_or_default(),
                key_host: effective_host(&e),
                needs_key: *id == "jev",
                key_present: key_present(vault, id, &e),
                candidates: d.candidates,
                threshold: d.threshold,
                timeout_ms: e.timeout_ms,
                order,
            })
        })
        .collect()
}

/// Lists every decision engine with its settings and whether a key is stored.
#[tauri::command]
pub async fn list_engines(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<EngineView>, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || Ok(views(&s.vault, &s.config.load()))).await
}

static CACHE: Mutex<Option<(Instant, Vec<EngineProbe>)>> = Mutex::new(None);

async fn probe_one(id: &'static str, e: &EngineEntry, key: bool) -> Option<EngineProbe> {
    let result = |status, detail| {
        Some(EngineProbe {
            id: id.into(),
            status,
            detail,
        })
    };
    if id == "llm" {
        return None;
    }
    if id == "jev" && !key {
        return result("no_key", None);
    }
    let url = e.base_url.as_deref().filter(|u| !u.is_empty());
    if !e.enabled && url.is_none() {
        return None;
    }
    let Some(base_url) = url.and_then(|u| Url::parse(u).ok()) else {
        return result("not_running", Some("no base URL".into()));
    };
    let engine = SystemOneEngine {
        id,
        base_url,
        key_host: None,
        bearer: None,
        timeout: PROBE_TIMEOUT,
    };
    let r = engine.probe().await;
    let status = if r.reachable {
        "reachable"
    } else {
        "not_running"
    };
    result(status, r.detail)
}

/// Probes enabled-or-configured engines concurrently; cached for 60 s.
#[tauri::command]
pub async fn probe_decision_engines(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<EngineProbe>, CommandError> {
    if let Some((t, v)) = &*CACHE.lock()
        && t.elapsed() < CACHE_TTL
    {
        return Ok(v.clone());
    }
    let cfg = state.config.load_full();
    let s = Arc::clone(&state);
    let c = Arc::clone(&cfg);
    let keys = blocking(move || {
        Ok(ENGINES.map(|(id, _)| {
            c.decision
                .engines
                .get(id)
                .is_some_and(|e| key_present(&s.vault, id, e))
        }))
    })
    .await?;
    let tasks = ENGINES.iter().zip(keys).map(|((id, _), key)| {
        let e = cfg.decision.engines.get(*id).cloned().unwrap_or_default();
        async move { probe_one(id, &e, key).await }
    });
    let mut out: Vec<EngineProbe> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .flatten()
        .collect();
    let llm_ready = cfg.providers.entries.values().any(|p| p.enabled);
    out.push(EngineProbe {
        id: "llm".into(),
        status: if llm_ready {
            "reachable"
        } else {
            "no_provider"
        },
        detail: None,
    });
    *CACHE.lock() = Some((Instant::now(), out.clone()));
    Ok(out)
}

/// Fields `set_engine` may change.
#[derive(Debug, Default)]
pub(crate) struct Patch {
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub candidates: Option<u32>,
    pub threshold: Option<f64>,
    pub timeout_ms: Option<u64>,
}

fn bad(code: &'static str, msg: &'static str) -> CommandError {
    CommandError::new(code, msg)
}

fn invalid_url() -> CommandError {
    bad(
        "invalid_url",
        "base URL must be https (http only for localhost)",
    )
}

/// Applies the patch and deletes an orphaned key at once. Tests only: the command saves first.
#[cfg(test)]
pub(crate) fn apply_patch(
    vault: &SecretVault,
    cfg: &mut Config,
    id: &str,
    patch: Patch,
) -> Result<(), CommandError> {
    for n in plan(vault, cfg, id, patch)? {
        vault
            .delete(&n)
            .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
    }
    Ok(())
}

/// Applies `patch` to engine `id`. On a host change the key bound to the old host is
/// orphaned (ADR-011 rule 6) and returned for deletion once the config is saved; the binding
/// moves to the new host.
pub(crate) fn plan(
    vault: &SecretVault,
    cfg: &mut Config,
    id: &str,
    patch: Patch,
) -> Result<Vec<SecretName>, CommandError> {
    let mut doomed = Vec::new();
    if !ENGINES.iter().any(|(i, _)| *i == id) {
        return Err(bad("unknown_engine", "no such engine"));
    }
    if patch.candidates.is_some_and(|c| !(1..=8).contains(&c)) {
        return Err(bad("invalid_candidates", "candidates is 1 to 8"));
    }
    if patch.threshold.is_some_and(|t| !(0.0..=1.0).contains(&t)) {
        return Err(bad("invalid_threshold", "threshold is 0 to 1"));
    }
    if patch
        .timeout_ms
        .is_some_and(|t| !(100..=60_000).contains(&t))
    {
        return Err(bad("invalid_timeout", "timeout is 100 to 60000 ms"));
    }
    let mut new = cfg.decision.engines.get(id).cloned().unwrap_or_default();
    let old = new.clone();
    if let Some(u) = patch.base_url {
        if id == "llm" {
            return Err(bad("unsupported", "this engine has no base URL"));
        }
        let u = u.trim().to_owned();
        if u.is_empty() {
            new.key_host = None;
        } else {
            let url = Url::parse(&u).map_err(|_| invalid_url())?;
            let host = url.host_str().ok_or_else(invalid_url)?.to_owned();
            http::check_url(&url, &host).map_err(|_| invalid_url())?;
            new.key_host = Some(host);
        }
        new.base_url = Some(u);
        let old_host = effective_host(&old);
        if old_host != new.key_host
            && let Some(Ok(name)) = old_host.map(|h| SecretName::parse(&format!("engine.{id}@{h}")))
        {
            doomed.push(name);
        }
    }
    if let Some(v) = patch.enabled {
        if v && id == "jev" && !key_present(vault, id, &new) {
            return Err(bad("key_needed", "add a key before enabling Jev"));
        }
        if v && id == "custom" && new.base_url.as_deref().is_none_or(|u| u.trim().is_empty()) {
            return Err(bad("url_needed", "set a base URL before enabling Custom"));
        }
        new.enabled = v;
    }
    if let Some(t) = patch.timeout_ms {
        new.timeout_ms = Some(t);
    }
    if let Some(c) = patch.candidates {
        cfg.decision.candidates = c;
    }
    if let Some(t) = patch.threshold {
        cfg.decision.threshold = t;
    }
    cfg.decision.engines.insert(id.to_owned(), new);
    Ok(doomed)
}

/// Patches one engine (or the global candidates/threshold), saves the config and returns
/// the refreshed list.
#[tauri::command]
pub async fn set_engine(
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: Option<bool>,
    base_url: Option<String>,
    candidates: Option<u32>,
    threshold: Option<f64>,
    timeout_ms: Option<u64>,
) -> Result<Vec<EngineView>, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        let patch = Patch {
            enabled,
            base_url,
            candidates,
            threshold,
            timeout_ms,
        };
        let doomed = s.update_config(|cfg| plan(&s.vault, cfg, &id, patch))?;
        // Only after the save: a failed save must not lose the stored key.
        for n in doomed {
            s.vault
                .delete(&n)
                .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
        }
        *CACHE.lock() = None;
        Ok(views(&s.vault, &s.config.load()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::secret::{MemoryBackend, MemoryMeta};

    fn vault() -> SecretVault {
        SecretVault::new(
            Arc::new(MemoryBackend::default()),
            Arc::new(MemoryMeta::default()),
        )
    }

    fn url(u: &str) -> Patch {
        Patch {
            base_url: Some(u.into()),
            ..Default::default()
        }
    }

    fn jev_key(v: &SecretVault) -> SecretName {
        let n = SecretName::parse("engine.jev@api.typesafe.ai").unwrap();
        v.set(&n, "jv-fake-0123456789abcdef".to_owned().into())
            .unwrap();
        n
    }

    #[test]
    fn views_cover_all_engines_off() {
        let all = views(&vault(), &Config::default());
        assert_eq!(all.len(), 5);
        assert!(all.iter().all(|v| !v.enabled && !v.key_present));
        assert!(all.iter().any(|v| v.id == "jev" && v.needs_key));
    }

    #[test]
    fn rejects_bad_values() {
        let (v, mut cfg) = (vault(), Config::default());
        let mut code = |p: Patch, id: &str| apply_patch(&v, &mut cfg, id, p).unwrap_err().code;
        for b in ["http://example.org", "nonsense", "ftp://x.test"] {
            assert_eq!(code(url(b), "kev"), "invalid_url");
        }
        assert_eq!(code(url("http://localhost:1"), "llm"), "unsupported");
        assert_eq!(code(url("http://localhost:1"), "zzz"), "unknown_engine");
        for c in [0, 9] {
            let p = Patch {
                candidates: Some(c),
                ..Default::default()
            };
            assert_eq!(code(p, "laya"), "invalid_candidates");
        }
        for t in [-0.1, 1.1] {
            let p = Patch {
                threshold: Some(t),
                ..Default::default()
            };
            assert_eq!(code(p, "laya"), "invalid_threshold");
        }
        let p = Patch {
            timeout_ms: Some(0),
            ..Default::default()
        };
        assert_eq!(code(p, "laya"), "invalid_timeout");
        assert!(apply_patch(&v, &mut cfg, "laya", url("http://127.0.0.1:8000")).is_ok());
    }

    #[test]
    fn jev_needs_key_to_enable() {
        let (v, mut cfg) = (vault(), Config::default());
        let on = || Patch {
            enabled: Some(true),
            ..Default::default()
        };
        let e = apply_patch(&v, &mut cfg, "jev", on()).unwrap_err();
        assert_eq!(e.code, "key_needed");
        jev_key(&v);
        apply_patch(&v, &mut cfg, "jev", on()).unwrap();
        assert!(cfg.decision.engines["jev"].enabled);
    }

    #[test]
    fn custom_needs_url_to_enable() {
        let (v, mut cfg) = (vault(), Config::default());
        let on = || Patch {
            enabled: Some(true),
            ..Default::default()
        };
        let e = apply_patch(&v, &mut cfg, "custom", on()).unwrap_err();
        assert_eq!(e.code, "url_needed");
        apply_patch(&v, &mut cfg, "custom", url("https://engine.example.org")).unwrap();
        apply_patch(&v, &mut cfg, "custom", on()).unwrap();
        assert!(cfg.decision.engines["custom"].enabled);
    }

    #[test]
    fn host_change_deletes_key() {
        let (v, mut cfg) = (vault(), Config::default());
        let name = jev_key(&v);
        apply_patch(&v, &mut cfg, "jev", url("https://example.org")).unwrap();
        assert!(!v.info(&name).unwrap().present);
        assert_eq!(
            cfg.decision.engines["jev"].key_host.as_deref(),
            Some("example.org")
        );
    }

    #[test]
    fn same_host_keeps_key_and_globals_apply() {
        let (v, mut cfg) = (vault(), Config::default());
        let name = jev_key(&v);
        let p = Patch {
            base_url: Some("https://api.typesafe.ai/x".into()),
            candidates: Some(3),
            threshold: Some(0.9),
            ..Default::default()
        };
        apply_patch(&v, &mut cfg, "jev", p).unwrap();
        assert!(v.info(&name).unwrap().present);
        assert_eq!((cfg.decision.candidates, cfg.decision.threshold), (3, 0.9));
    }
}
