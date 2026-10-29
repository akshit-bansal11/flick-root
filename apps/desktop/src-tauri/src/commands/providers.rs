//! Provider list, probing and configuration commands.
use super::blocking;
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::config::{Config, ProviderEntry};
use providers::presets::{Group, PRESETS, Preset, build_provider, preset};
use providers::secret::{SecretInfo, SecretName, SecretVault};
use providers::{ModelInfo, ProbeResult, ProviderConfig, ProviderError, Wire, http};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tauri::State;
use url::Url;

/// One provider as the settings UI sees it. Carries no secret value.
#[derive(Debug, Serialize)]
pub struct ProviderView {
    /// Provider id.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Picker group: `local`, `free` or `own_key`.
    pub group: &'static str,
    /// Enabled by the user.
    pub enabled: bool,
    /// Base URL.
    pub base_url: String,
    /// Model name.
    pub model: String,
    /// Extra models offered as thinking modes.
    pub models: Vec<String>,
    /// Whether a key is required.
    pub key_required: bool,
    /// Stored key info, if any.
    pub key: Option<SecretInfo>,
    /// Whether `reasoning_effort` may be sent.
    pub supports_effort: bool,
    /// `off`, `key_needed` or `ready`.
    pub status: &'static str,
}

/// Preset for `id`; unknown ids use the last preset, `custom`.
pub(crate) fn preset_of(id: &str) -> &'static Preset {
    preset(id).unwrap_or(&PRESETS[PRESETS.len() - 1])
}

/// Config entry for `id`, or one synthesized from the preset; `None` for unknown ids.
pub(crate) fn entry_of(cfg: &Config, id: &str) -> Option<ProviderEntry> {
    if let Some(e) = cfg.providers.entries.get(id) {
        return Some(e.clone());
    }
    let pr = preset(id)?;
    Some(ProviderEntry {
        wire: match pr.wire {
            Wire::OpenAi => "openai",
            Wire::Anthropic => "anthropic",
        }
        .into(),
        base_url: pr.default_base_url.into(),
        ..Default::default()
    })
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_owned)
}

/// Host a key for provider `id` must be bound to: the configured `key_host`, else the
/// preset's host, else the entry's base URL host (custom).
pub(crate) fn expected_host(cfg: &Config, id: &str) -> Option<String> {
    let e = entry_of(cfg, id)?;
    e.key_host
        .clone()
        .or_else(|| host_of(preset_of(id).default_base_url))
        .or_else(|| host_of(&e.base_url))
}

fn provider_cfg(id: &str, e: &ProviderEntry) -> Option<ProviderConfig> {
    let pr = preset_of(id);
    Some(ProviderConfig {
        id: id.to_owned(),
        wire: pr.wire,
        base_url: Url::parse(&e.base_url).ok()?,
        model: e.model.clone(),
        key_host: e.key_host.clone(),
        supports_effort: pr.supports_effort,
    })
}

fn view(vault: &SecretVault, cfg: &Config, id: &str, e: &ProviderEntry) -> ProviderView {
    let pr = preset_of(id);
    let key = expected_host(cfg, id)
        .and_then(|h| SecretName::provider(id, &h).ok())
        .and_then(|n| vault.info(&n).ok())
        .filter(|i| i.present);
    let status = if !e.enabled {
        "off"
    } else if pr.key_required && key.is_none() {
        "key_needed"
    } else {
        "ready"
    };
    ProviderView {
        id: id.to_owned(),
        title: pr.title.to_owned(),
        group: match pr.group {
            Group::Local => "local",
            Group::FreeGateway => "free",
            Group::OwnKey => "own_key",
        },
        enabled: e.enabled,
        base_url: e.base_url.clone(),
        model: e.model.clone(),
        models: e.models.clone(),
        key_required: pr.key_required,
        key,
        supports_effort: pr.supports_effort,
        status,
    }
}

/// Config order first, then remaining presets, then remaining config entries.
fn ordered_ids(cfg: &Config) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let extra = PRESETS
        .iter()
        .map(|p| p.id.to_owned())
        .chain(cfg.providers.entries.keys().cloned());
    for id in cfg.providers.order.iter().cloned().chain(extra) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn views(vault: &SecretVault, cfg: &Config) -> Vec<ProviderView> {
    ordered_ids(cfg)
        .iter()
        .filter_map(|id| Some(view(vault, cfg, id, &entry_of(cfg, id)?)))
        .collect()
}

/// Lists every provider with its key info and status.
#[tauri::command]
pub async fn list_providers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderView>, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || Ok(views(&s.vault, &s.config.load()))).await
}

/// Probes local servers and enabled keyed providers; never returns a key.
#[tauri::command]
pub async fn probe_providers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProbeResult>, CommandError> {
    let t0 = Instant::now();
    let found = providers::detect::detect_local(providers::detect::PROBE_TIMEOUT).await;
    let ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut out: Vec<ProbeResult> = PRESETS
        .iter()
        .filter(|p| p.group == Group::Local)
        .map(|p| {
            let hit = found.iter().find(|d| d.preset_id == p.id);
            ProbeResult {
                id: p.id.to_owned(),
                reachable: hit.is_some(),
                models: hit.map(|d| d.models.clone()).unwrap_or_default(),
                ms,
                error: None,
            }
        })
        .collect();
    let cfg = state.config.load_full();
    // ponytail: sequential; run keyed probes concurrently if many are enabled.
    for id in ordered_ids(&cfg) {
        let pr = preset_of(&id);
        let Some(e) = entry_of(&cfg, &id).filter(|e| e.enabled && pr.key_required) else {
            continue;
        };
        match build_provider(&id, &e, pr, state.keys.clone()) {
            Ok(p) => out.push(p.probe().await),
            Err(err) => out.push(ProbeResult {
                id,
                reachable: false,
                models: vec![],
                ms: 0,
                error: Some(err.to_string()),
            }),
        }
    }
    Ok(out)
}

const MAX_MODELS: usize = 20;
const MAX_MODEL_LEN: usize = 100;

/// Fields `set_provider` may change.
#[derive(Debug, Default)]
pub(crate) struct Patch {
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub models: Option<Vec<String>>,
    pub effort: Option<String>,
}

fn invalid_url() -> CommandError {
    CommandError::new(
        "invalid_url",
        "base URL must be https (http only for localhost)",
    )
}

/// Whether the base-URL host differs; the key binding then moves to the new host even
/// when no key was stored for the old one.
fn host_changed(old: &ProviderConfig, new: &ProviderConfig) -> bool {
    old.base_url.host_str() != new.base_url.host_str()
}

/// Applies `patch` to provider `id` in `cfg`. On a host change the key bound to the old
/// host is deleted (ADR-011 rule 6) and the binding moves to the new host. Returns true
/// when the user must enter a key again.
pub(crate) fn apply_patch(
    vault: &SecretVault,
    cfg: &mut Config,
    id: &str,
    patch: Patch,
) -> Result<bool, CommandError> {
    let old = entry_of(cfg, id)
        .ok_or_else(|| CommandError::new("unknown_provider", "no such provider"))?;
    let mut new = old.clone();
    if let Some(v) = patch.enabled {
        new.enabled = v;
    }
    if let Some(m) = patch.model {
        new.model = m.trim().to_owned();
    }
    if let Some(ms) = patch.models {
        new.models = ms
            .iter()
            .map(|m| m.trim().chars().take(MAX_MODEL_LEN).collect::<String>())
            .filter(|m| !m.is_empty())
            .take(MAX_MODELS)
            .collect();
    }
    if let Some(e) = patch.effort {
        new.effort = match e.as_str() {
            "" => None,
            "fast" | "think" => Some(e),
            _ => {
                return Err(CommandError::new(
                    "invalid_effort",
                    "effort is fast or think",
                ));
            }
        };
    }
    let mut key_needed = false;
    if let Some(u) = patch.base_url {
        let u = u.trim().to_owned();
        let url = Url::parse(&u).map_err(|_| invalid_url())?;
        let host = url.host_str().ok_or_else(invalid_url)?.to_owned();
        http::check_url(&url, &host).map_err(|_| invalid_url())?;
        new.base_url = u;
        if let (Some(o), Some(n)) = (provider_cfg(id, &old), provider_cfg(id, &new)) {
            key_needed = http::on_base_url_change(vault, &o, &n)
                .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
            if host_changed(&o, &n) {
                new.key_host = Some(host);
            }
        }
    }
    cfg.providers.entries.insert(id.to_owned(), new);
    if !cfg.providers.order.iter().any(|i| i == id) {
        cfg.providers.order.push(id.to_owned());
    }
    Ok(key_needed)
}

/// Patches one provider, saves the config and rebuilds the hub. A host change deletes
/// the old key; the returned view then has status `key_needed`.
#[tauri::command]
pub async fn set_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: Option<bool>,
    base_url: Option<String>,
    model: Option<String>,
    models: Option<Vec<String>>,
    effort: Option<String>,
) -> Result<ProviderView, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        let patch = Patch {
            enabled,
            base_url,
            model,
            models,
            effort,
        };
        s.update_config(|cfg| apply_patch(&s.vault, cfg, &id, patch))?;
        let cfg = s.config.load();
        let e = entry_of(&cfg, &id)
            .ok_or_else(|| CommandError::new("unknown_provider", "no such provider"))?;
        Ok(view(&s.vault, &cfg, &id, &e))
    })
    .await
}

/// Writes a fetched model list to provider `id`: the first id becomes `model` when none is
/// set, the rest replace `models` (capped), and `efforts` is replaced by the fetched map.
/// `enabled` is unchanged. An empty list is an error and leaves `cfg` untouched.
pub(crate) fn apply_models(
    cfg: &mut Config,
    id: &str,
    fetched: Vec<ModelInfo>,
) -> Result<(), CommandError> {
    let clean = |m: &str| m.trim().chars().take(MAX_MODEL_LEN).collect::<String>();
    let fetched: Vec<ModelInfo> = fetched
        .into_iter()
        .map(|m| ModelInfo {
            id: clean(&m.id),
            efforts: m.efforts,
        })
        .filter(|m| !m.id.is_empty())
        .collect();
    let Some(first) = fetched.first() else {
        return Err(CommandError::new(
            "no_models",
            "the server listed no models",
        ));
    };
    let mut e = entry_of(cfg, id)
        .ok_or_else(|| CommandError::new("unknown_provider", "no such provider"))?;
    if e.model.trim().is_empty() {
        e.model = first.id.clone();
    }
    let mut models: Vec<String> = Vec::new();
    for m in &fetched {
        if m.id != e.model.trim() && !models.contains(&m.id) {
            models.push(m.id.clone());
        }
    }
    models.truncate(MAX_MODELS);
    e.models = models;
    e.efforts = fetched.into_iter().map(|m| (m.id, m.efforts)).collect();
    cfg.providers.entries.insert(id.to_owned(), e);
    if !cfg.providers.order.iter().any(|i| i == id) {
        cfg.providers.order.push(id.to_owned());
    }
    Ok(())
}

/// Maps a model-fetch failure to a UI error code; the message never holds a key or body.
fn fetch_err(e: ProviderError) -> CommandError {
    let code = match e {
        ProviderError::NoKey { .. } => "no_key",
        ProviderError::HostMismatch { .. } => "host_mismatch",
        _ => "network",
    };
    CommandError::new(code, e.to_string())
}

/// Reads provider `id`'s model list and effort levels from its server, then saves them.
/// A failed fetch leaves the config untouched.
#[tauri::command]
pub async fn refresh_provider_models(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<ProviderView, CommandError> {
    let cfg = state.config.load_full();
    let mut entry = entry_of(&cfg, &id)
        .ok_or_else(|| CommandError::new("unknown_provider", "no such provider"))?;
    entry.key_host = entry.key_host.or_else(|| expected_host(&cfg, &id));
    let p = build_provider(&id, &entry, preset_of(&id), state.keys.clone())
        .map_err(|e| CommandError::new("network", e.to_string()))?;
    let fetched = p.fetch_models().await.map_err(fetch_err)?;
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|cfg| apply_models(cfg, &id, fetched))?;
        let cfg = s.config.load();
        let e = entry_of(&cfg, &id)
            .ok_or_else(|| CommandError::new("unknown_provider", "no such provider"))?;
        Ok(view(&s.vault, &cfg, &id, &e))
    })
    .await
}

fn apply_order(cfg: &mut Config, ids: Vec<String>) -> Result<(), CommandError> {
    if let Some(bad) = ids.iter().find(|i| entry_of(cfg, i).is_none()) {
        return Err(CommandError::new(
            "unknown_provider",
            format!("no such provider: {bad}"),
        ));
    }
    for id in &ids {
        if let Some(e) = entry_of(cfg, id) {
            cfg.providers.entries.entry(id.clone()).or_insert(e);
        }
    }
    cfg.providers.order = ids;
    Ok(())
}

/// Sets the fallback order.
#[tauri::command]
pub async fn set_provider_order(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<(), CommandError> {
    let s = Arc::clone(&state);
    blocking(move || s.update_config(|cfg| apply_order(cfg, ids))).await
}

/// One selectable provider and model pair.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingMode {
    /// `<provider_id>/<model>`.
    pub id: String,
    /// Provider id.
    pub provider_id: String,
    /// Provider display title.
    pub provider_label: String,
    /// Model name.
    pub model: String,
    /// Runs on this machine.
    pub local: bool,
    /// Effort levels this model accepts, low to max; empty when none.
    pub efforts: Vec<String>,
}

/// Modes plus the current pick.
#[derive(Debug, Serialize)]
pub struct ThinkingModes {
    /// Every mode, in fallback order.
    pub modes: Vec<ThinkingMode>,
    /// Chosen mode id; the first mode when none or stale.
    pub selected: Option<String>,
    /// Stored effort: `auto` or a level.
    pub effort: String,
}

const DEFAULT_EFFORTS: [&str; 3] = ["low", "medium", "high"];

/// Levels `model` supports: none when the preset cannot send effort, the fetched list when
/// present, else the default three.
fn efforts_for(e: &ProviderEntry, pr: &Preset, model: &str) -> Vec<String> {
    if !pr.supports_effort {
        return Vec::new();
    }
    match e.efforts.get(model) {
        Some(list) => list.clone(),
        None => DEFAULT_EFFORTS.iter().map(|l| (*l).to_owned()).collect(),
    }
}

/// The effort to send: `effort` as a level, only if the chosen model (else the first) lists it.
pub(crate) fn effective_effort(
    cfg: &Config,
    thinking_mode: Option<&str>,
    effort: &str,
) -> Option<String> {
    let level = providers::parse_effort(effort)?;
    let modes = thinking_modes(cfg);
    let mode = thinking_mode
        .and_then(|id| modes.iter().find(|m| m.id == id))
        .or_else(|| modes.first())?;
    mode.efforts
        .iter()
        .any(|l| l == level)
        .then(|| level.to_owned())
}

/// Splits a mode id at the first `/`.
pub(crate) fn parse_mode_id(id: &str) -> Option<(&str, &str)> {
    id.split_once('/')
        .filter(|(p, m)| !p.is_empty() && !m.is_empty())
}

/// One mode per model of each enabled provider, in fallback order.
pub(crate) fn thinking_modes(cfg: &Config) -> Vec<ThinkingMode> {
    let mut out = Vec::new();
    for pid in &cfg.providers.order {
        let Some(e) = cfg.providers.entries.get(pid).filter(|e| e.enabled) else {
            continue;
        };
        let pr = preset_of(pid);
        let mut seen: Vec<&str> = Vec::new();
        for m in std::iter::once(&e.model).chain(&e.models) {
            let m = m.trim();
            if m.is_empty() || seen.contains(&m) {
                continue;
            }
            seen.push(m);
            out.push(ThinkingMode {
                id: format!("{pid}/{m}"),
                provider_id: pid.clone(),
                provider_label: pr.title.to_owned(),
                model: m.to_owned(),
                local: pr.group == Group::Local,
                efforts: efforts_for(e, pr, m),
            });
        }
    }
    out
}

/// The stored id if still valid, else the first mode's id.
fn selected_mode(cfg: &Config, modes: &[ThinkingMode]) -> Option<String> {
    let stored = cfg.modes.query.thinking_mode.as_deref();
    modes
        .iter()
        .find(|m| Some(m.id.as_str()) == stored)
        .or_else(|| modes.first())
        .map(|m| m.id.clone())
}

/// The valid (provider id, model) behind a mode id, if any.
pub(crate) fn resolve_mode(cfg: &Config, id: &str) -> Option<(String, String)> {
    let (p, m) = parse_mode_id(id)?;
    thinking_modes(cfg)
        .into_iter()
        .find(|t| t.provider_id == p && t.model == m)
        .map(|t| (t.provider_id, t.model))
}

/// Lists thinking modes and the selected one; carries no key data.
#[tauri::command]
pub fn list_thinking_modes(state: State<'_, Arc<AppState>>) -> ThinkingModes {
    let cfg = state.config.load();
    let modes = thinking_modes(&cfg);
    let selected = selected_mode(&cfg, &modes);
    let effort = providers::parse_effort(&cfg.modes.query.effort)
        .unwrap_or("auto")
        .to_owned();
    ThinkingModes {
        modes,
        selected,
        effort,
    }
}

/// Persists the global effort: `auto` or a level.
#[tauri::command]
pub async fn set_effort(
    state: State<'_, Arc<AppState>>,
    level: String,
) -> Result<(), CommandError> {
    let level = if level == "auto" {
        level
    } else {
        providers::parse_effort(&level)
            .ok_or_else(|| CommandError::new("invalid_effort", "effort is auto or a level"))?
            .to_owned()
    };
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|cfg| {
            cfg.modes.query.effort = level;
            Ok(())
        })
    })
    .await
}

/// Persists the chosen thinking mode; unknown ids are rejected.
#[tauri::command]
pub async fn set_thinking_mode(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|cfg| {
            resolve_mode(cfg, &id)
                .ok_or_else(|| CommandError::new("unknown_mode", "no such thinking mode"))?;
            cfg.modes.query.thinking_mode = Some(id);
            Ok(())
        })
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

    fn keyed(v: &SecretVault) -> SecretName {
        let name = SecretName::provider("openrouter", "openrouter.ai").unwrap();
        v.set(
            &name,
            concat!("sk-o", "r-fake-0123456789abcdef").to_owned().into(),
        )
        .unwrap();
        name
    }

    fn url_patch(u: &str) -> Patch {
        Patch {
            base_url: Some(u.into()),
            ..Default::default()
        }
    }

    #[test]
    fn host_change_deletes_key() {
        let v = vault();
        let name = keyed(&v);
        let mut cfg = Config::default();
        let p = url_patch("https://example.org/v1");
        assert!(apply_patch(&v, &mut cfg, "openrouter", p).unwrap());
        assert!(!v.info(&name).unwrap().present);
        assert_eq!(
            cfg.providers.entries["openrouter"].key_host.as_deref(),
            Some("example.org")
        );
        let e = entry_of(&cfg, "openrouter").unwrap();
        assert!(view(&v, &cfg, "openrouter", &e).key.is_none());
    }

    #[test]
    fn host_change_without_key_still_binds_new_host() {
        let v = vault();
        let mut cfg = Config::default();
        apply_patch(&v, &mut cfg, "ollama", url_patch("http://localhost:1/v1")).unwrap();
        apply_patch(&v, &mut cfg, "ollama", url_patch("http://127.0.0.1:2/v1")).unwrap();
        assert_eq!(
            cfg.providers.entries["ollama"].key_host.as_deref(),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn same_host_keeps_key_and_bad_urls_rejected() {
        let v = vault();
        let name = keyed(&v);
        let mut cfg = Config::default();
        let p = url_patch("https://openrouter.ai/other");
        assert!(!apply_patch(&v, &mut cfg, "openrouter", p).unwrap());
        assert!(v.info(&name).unwrap().present);
        for bad in ["http://example.org/v1", "nonsense", "ftp://x.test"] {
            let e = apply_patch(&v, &mut cfg, "openrouter", url_patch(bad)).unwrap_err();
            assert_eq!(e.code, "invalid_url");
        }
        let lo = url_patch("http://localhost:1/v1");
        assert!(apply_patch(&v, &mut cfg, "ollama", lo).is_ok());
    }

    #[test]
    fn order_rejects_unknown() {
        let mut cfg = Config::default();
        let e = apply_order(&mut cfg, vec!["nope".into()]).unwrap_err();
        assert_eq!(e.code, "unknown_provider");
        apply_order(&mut cfg, vec!["openai".into(), "ollama".into()]).unwrap();
        assert_eq!(cfg.providers.order, ["openai", "ollama"]);
    }

    #[test]
    fn views_cover_presets_and_start_off() {
        let all = views(&vault(), &Config::default());
        assert_eq!(all.len(), PRESETS.len());
        assert!(all.iter().all(|v| v.status == "off"));
    }

    #[test]
    fn thinking_modes_filter_order_dedupe() {
        let mut cfg = Config::default();
        assert!(thinking_modes(&cfg).is_empty());
        apply_order(
            &mut cfg,
            vec!["openai".into(), "ollama".into(), "anthropic".into()],
        )
        .unwrap();
        for (id, model, extra) in [
            ("openai", "gpt-a", vec!["gpt-b", "gpt-a", " "]),
            ("ollama", "", vec!["llama"]),
        ] {
            let e = cfg.providers.entries.get_mut(id).unwrap();
            e.enabled = true;
            e.model = model.into();
            e.models = extra.into_iter().map(String::from).collect();
        }
        let ids: Vec<_> = thinking_modes(&cfg).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["openai/gpt-a", "openai/gpt-b", "ollama/llama"]);
        assert!(thinking_modes(&cfg)[2].local);
        assert_eq!(parse_mode_id("a/b/c"), Some(("a", "b/c")));
        assert_eq!(parse_mode_id("nope"), None);
        assert!(resolve_mode(&cfg, "openai/gpt-b").is_some());
        assert!(resolve_mode(&cfg, "anthropic/x").is_none());
        let modes = thinking_modes(&cfg);
        assert_eq!(selected_mode(&cfg, &modes).as_deref(), Some("openai/gpt-a"));
        cfg.modes.query.thinking_mode = Some("ollama/llama".into());
        assert_eq!(selected_mode(&cfg, &modes).as_deref(), Some("ollama/llama"));
        cfg.modes.query.thinking_mode = Some("gone/x".into());
        assert_eq!(selected_mode(&cfg, &modes).as_deref(), Some("openai/gpt-a"));
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn efforts_for_cases() {
        let mut e = ProviderEntry::default();
        assert!(!preset_of("ollama").supports_effort);
        assert!(efforts_for(&e, preset_of("ollama"), "m").is_empty());
        let pr = preset_of("openai");
        assert!(pr.supports_effort);
        assert_eq!(efforts_for(&e, pr, "m"), strs(&["low", "medium", "high"]));
        e.efforts.insert("m".into(), Vec::new());
        assert!(efforts_for(&e, pr, "m").is_empty());
        e.efforts.insert("m".into(), strs(&["low", "max"]));
        assert_eq!(efforts_for(&e, pr, "m"), strs(&["low", "max"]));
    }

    fn effort_cfg() -> Config {
        let mut cfg = Config::default();
        apply_order(&mut cfg, vec!["openai".into(), "ollama".into()]).unwrap();
        let o = cfg.providers.entries.get_mut("openai").unwrap();
        o.enabled = true;
        o.model = "a".into();
        o.models = vec!["b".into()];
        o.efforts.insert("a".into(), strs(&["low", "high"]));
        o.efforts.insert("b".into(), strs(&["medium"]));
        let l = cfg.providers.entries.get_mut("ollama").unwrap();
        l.enabled = true;
        l.model = "l".into();
        cfg
    }

    #[test]
    fn modes_carry_efforts_and_effective_effort_filters() {
        let cfg = effort_cfg();
        let modes = thinking_modes(&cfg);
        assert_eq!(modes[0].efforts, strs(&["low", "high"]));
        assert!(modes[2].efforts.is_empty());
        assert_eq!(
            effective_effort(&cfg, Some("openai/a"), "low").as_deref(),
            Some("low")
        );
        assert_eq!(effective_effort(&cfg, Some("openai/a"), "max"), None);
        assert_eq!(effective_effort(&cfg, Some("openai/a"), "fast"), None);
        assert_eq!(
            effective_effort(&cfg, Some("openai/a"), "think").as_deref(),
            Some("high")
        );
        assert_eq!(effective_effort(&cfg, Some("ollama/l"), "low"), None);
        assert_eq!(
            effective_effort(&cfg, Some("gone/x"), "high").as_deref(),
            Some("high")
        );
        assert_eq!(effective_effort(&cfg, None, "low").as_deref(), Some("low"));
    }

    fn info(id: &str, efforts: &[&str]) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            efforts: strs(efforts),
        }
    }

    #[test]
    fn apply_models_fills_empty_model_and_writes_efforts() {
        let mut cfg = Config::default();
        cfg.providers.order.clear();
        assert!(cfg.providers.entries["openai"].model.is_empty());
        apply_models(
            &mut cfg,
            "openai",
            vec![
                info("a", &["low", "max"]),
                info("b", &[]),
                info("a", &["low"]),
            ],
        )
        .unwrap();
        let e = &cfg.providers.entries["openai"];
        assert_eq!(e.model, "a");
        assert_eq!(e.models, ["b"]);
        assert_eq!(e.efforts["a"], strs(&["low"]));
        assert!(e.efforts["b"].is_empty());
        assert!(!e.enabled);
        assert_eq!(cfg.providers.order, ["openai"]);
    }

    #[test]
    fn apply_models_keeps_set_model_and_enabled() {
        let mut cfg = effort_cfg();
        apply_models(
            &mut cfg,
            "openai",
            vec![info("x", &["low"]), info("a", &["high"])],
        )
        .unwrap();
        let e = &cfg.providers.entries["openai"];
        assert_eq!(e.model, "a");
        assert_eq!(e.models, ["x"]);
        assert!(e.enabled);
        assert_eq!(e.efforts.len(), 2);
        let modes = thinking_modes(&cfg);
        assert_eq!(modes[0].id, "openai/a");
        assert_eq!(modes[0].efforts, strs(&["high"]));
        assert_eq!(modes[1].efforts, strs(&["low"]));
    }

    #[test]
    fn apply_models_caps_extra_models() {
        let mut cfg = Config::default();
        let many: Vec<_> = (0..30).map(|i| info(&format!("m{i}"), &[])).collect();
        apply_models(&mut cfg, "openai", many).unwrap();
        let e = &cfg.providers.entries["openai"];
        assert_eq!(e.model, "m0");
        assert_eq!(e.models.len(), MAX_MODELS);
        assert!(!e.models.contains(&e.model));
    }

    #[test]
    fn apply_models_empty_is_error_and_leaves_cfg() {
        let mut cfg = effort_cfg();
        let before = cfg.clone();
        let e = apply_models(&mut cfg, "openai", vec![info(" ", &["low"])]).unwrap_err();
        assert_eq!(e.code, "no_models");
        assert_eq!(cfg, before);
        let e = apply_models(&mut cfg, "nope", vec![info("a", &[])]).unwrap_err();
        assert_eq!(e.code, "unknown_provider");
        assert_eq!(cfg, before);
    }

    #[test]
    fn fetch_errors_map_to_codes_without_secrets() {
        let e = fetch_err(ProviderError::NoKey { host: "h".into() });
        assert_eq!(e.code, "no_key");
        let e = fetch_err(ProviderError::HostMismatch {
            bound: "a".into(),
            actual: "b".into(),
        });
        assert_eq!(e.code, "host_mismatch");
        assert_eq!(
            fetch_err(ProviderError::Status { code: 401 }).code,
            "network"
        );
    }
}
