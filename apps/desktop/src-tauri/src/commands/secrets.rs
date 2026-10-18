//! Write-only secret commands (ADR-011): no command returns a value.
use super::blocking;
use super::providers::{entry_of, expected_host, preset_of};
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::config::Config;
use providers::presets::build_provider;
use providers::secret::{SecretInfo, SecretName, VaultError};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

fn vault_err(e: &VaultError) -> CommandError {
    let code = match e {
        VaultError::InvalidName => "invalid_name",
        VaultError::InvalidValue => "invalid_value",
        VaultError::NotFound => "not_found",
        _ => "store_failed",
    };
    CommandError::new(code, e.to_string())
}

/// Host a secret named `name` must be bound to, per config.
fn bound_host(cfg: &Config, name: &SecretName) -> Option<String> {
    match name.kind() {
        "provider" => expected_host(cfg, name.id()),
        "engine" => cfg
            .decision
            .engines
            .get(name.id())
            .and_then(|e| e.key_host.clone()),
        _ => None,
    }
}

/// Checks acknowledgement, name format and host binding, in that order.
pub(crate) fn validate_set(
    cfg: &Config,
    name: &str,
    acknowledged: bool,
) -> Result<SecretName, CommandError> {
    if !acknowledged {
        return Err(CommandError::new(
            "not_acknowledged",
            "confirm that the key is sent only to its host",
        ));
    }
    let name = SecretName::parse(name).map_err(|e| vault_err(&e))?;
    match bound_host(cfg, &name) {
        Some(h) if h.eq_ignore_ascii_case(name.host()) => Ok(name),
        _ => Err(CommandError::new(
            "host_unbound",
            "the name host does not match the configured host",
        )),
    }
}

/// Stores a key. The value is registered for log redaction, then moved into a
/// `SecretString` (zeroized on drop); nothing returns it.
#[tauri::command]
pub async fn set_secret(
    state: State<'_, Arc<AppState>>,
    name: String,
    value: String,
    acknowledged: bool,
) -> Result<SecretInfo, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        let name = validate_set(&s.config.load(), &name, acknowledged)?;
        let trimmed = value.trim().to_owned();
        drop(value);
        launcher_core::redact::global().register(trimmed.as_bytes());
        s.vault
            .set(&name, trimmed.into())
            .map_err(|e| vault_err(&e))?;
        if name.kind() == "provider" {
            bind_provider_host(&s, &name)?;
        }
        s.vault.info(&name).map_err(|e| vault_err(&e))
    })
    .await
}

/// Persists `key_host` for a provider whose entry has none, so requests can find the key.
fn bind_provider_host(s: &AppState, name: &SecretName) -> Result<(), CommandError> {
    let id = name.id();
    if s.config
        .load()
        .providers
        .entries
        .get(id)
        .is_some_and(|e| e.key_host.is_some())
    {
        return Ok(());
    }
    s.update_config(|cfg| {
        if let Some(mut e) = entry_of(cfg, id) {
            e.key_host = Some(name.host().to_owned());
            cfg.providers.entries.insert(id.to_owned(), e);
            if !cfg.providers.order.iter().any(|i| i == id) {
                cfg.providers.order.push(id.to_owned());
            }
        }
        Ok(())
    })
}

/// Deletes a key and its metadata.
#[tauri::command]
pub async fn delete_secret(
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<(), CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        let name = SecretName::parse(&name).map_err(|e| vault_err(&e))?;
        if !s.vault.info(&name).map_err(|e| vault_err(&e))?.present {
            return Err(CommandError::new("not_found", "no such secret"));
        }
        s.vault.delete(&name).map_err(|e| vault_err(&e))
    })
    .await
}

/// Outcome of `test_secret`; never carries a response body.
#[derive(Debug, Serialize)]
pub struct TestResult {
    /// Server accepted the key.
    pub ok: bool,
    /// HTTP status when the server answered.
    pub status: Option<u16>,
    /// Round-trip milliseconds.
    pub ms: u64,
}

fn parse_status(err: &str) -> Option<u16> {
    err.strip_prefix("server returned HTTP ")?
        .trim()
        .parse()
        .ok()
}

/// Probes the provider a key is bound to, using the key.
#[tauri::command]
pub async fn test_secret(
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<TestResult, CommandError> {
    let s = Arc::clone(&state);
    let n = SecretName::parse(&name).map_err(|e| vault_err(&e))?;
    let present = {
        let (s, n) = (Arc::clone(&s), n.clone());
        blocking(move || Ok(s.vault.info(&n).map_err(|e| vault_err(&e))?.present)).await?
    };
    if !present {
        return Err(CommandError::new("not_found", "no such secret"));
    }
    if n.kind() != "provider" {
        return Err(CommandError::new(
            "unsupported",
            "only provider keys can be tested",
        ));
    }
    let cfg = s.config.load_full();
    let id = n.id();
    let mut entry =
        entry_of(&cfg, id).ok_or_else(|| CommandError::new("not_found", "no such provider"))?;
    if !expected_host(&cfg, id).is_some_and(|h| h.eq_ignore_ascii_case(n.host())) {
        return Err(CommandError::new(
            "host_mismatch",
            "key is bound to another host",
        ));
    }
    entry.key_host = Some(n.host().to_owned());
    let p = build_provider(id, &entry, preset_of(id), s.keys.clone())
        .map_err(|e| CommandError::new("network", e.to_string()))?;
    let r = p.probe().await;
    match r.error {
        None => Ok(TestResult {
            ok: true,
            status: Some(200),
            ms: r.ms,
        }),
        Some(e) => match parse_status(&e) {
            Some(code) => Ok(TestResult {
                ok: false,
                status: Some(code),
                ms: r.ms,
            }),
            None if e.starts_with("key is bound") => Err(CommandError::new("host_mismatch", e)),
            None => Err(CommandError::new("network", e)),
        },
    }
}

/// Lists key metadata (name, last4 for long keys, host, times).
#[tauri::command]
pub async fn list_secrets(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SecretInfo>, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || s.vault.list().map_err(|e| vault_err(&e))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unacknowledged_first() {
        let e = validate_set(&Config::default(), "garbage", false).unwrap_err();
        assert_eq!(e.code, "not_acknowledged");
    }

    #[test]
    fn name_and_host_binding() {
        let cfg = Config::default();
        let code = |n: &str| validate_set(&cfg, n, true).unwrap_err().code;
        assert_eq!(code("nope"), "invalid_name");
        assert!(validate_set(&cfg, "provider.openrouter@openrouter.ai", true).is_ok());
        assert!(validate_set(&cfg, "provider.anthropic@api.anthropic.com", true).is_ok());
        assert_eq!(code("provider.openrouter@evil.test"), "host_unbound");
        assert_eq!(code("provider.nosuch@x.test"), "host_unbound");
        assert_eq!(code("integration.spotify@spotify.com"), "host_unbound");
    }

    #[test]
    fn secret_info_has_no_value_field() {
        let info = SecretInfo {
            name: "provider.x@x.test".into(),
            present: true,
            last4: Some("abcd".into()),
            host: "x.test".into(),
            added_at: Some(1),
            last_used: None,
        };
        let v = serde_json::to_value(&info).unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert!(
            keys.iter()
                .all(|k| !k.contains("value") && !k.contains("secret"))
        );
    }

    #[test]
    fn status_parse() {
        assert_eq!(parse_status("server returned HTTP 401"), Some(401));
        assert_eq!(parse_status("network error: x"), None);
    }
}
