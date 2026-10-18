//! Secret vault per ADR-011: write-only from the outside, bound to one host.
//!
//! There is no public getter that returns a secret. The only read path is the
//! [`VaultKeys`] key source used at request time (host-bound).

use crate::{KeySource, ProviderConfig, ProviderError};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Vault errors. Messages never carry secret values.
#[derive(thiserror::Error, Debug)]
pub enum VaultError {
    /// The secret name does not follow `<kind>.<id>@<host>`.
    #[error("invalid secret name")]
    InvalidName,
    /// The value is empty or spans several lines.
    #[error("secret value must be a single non-empty line")]
    InvalidValue,
    /// No secret is stored under this name.
    #[error("no such secret")]
    NotFound,
    /// The credential store failed.
    #[error("secret store error: {0}")]
    Backend(String),
    /// The metadata store failed.
    #[error("secret metadata error: {0}")]
    Meta(String),
}

impl From<VaultError> for ProviderError {
    fn from(e: VaultError) -> Self {
        match e {
            VaultError::NotFound => ProviderError::Decode("no such secret"),
            _ => ProviderError::Decode("secret store unavailable"),
        }
    }
}

/// A validated credential name: `<kind>.<id>@<host>`, all lowercase.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretName {
    full: String,
    kind_end: usize,
    at: usize,
}

const KINDS: [&str; 4] = ["provider", "engine", "integration", "test"];

fn slug_ok(s: &str, extra: &[char]) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(&c))
}

impl SecretName {
    /// Parse and validate a name such as `provider.openrouter@openrouter.ai`.
    pub fn parse(s: &str) -> Result<Self, VaultError> {
        let (left, host) = s.split_once('@').ok_or(VaultError::InvalidName)?;
        let (kind, id) = left.split_once('.').ok_or(VaultError::InvalidName)?;
        if !KINDS.contains(&kind) || !slug_ok(id, &['.', '-', '_']) || !slug_ok(host, &['.', '-']) {
            return Err(VaultError::InvalidName);
        }
        Ok(Self {
            full: s.to_owned(),
            kind_end: kind.len(),
            at: left.len(),
        })
    }

    /// Name for a provider key bound to `host`.
    pub fn provider(id: &str, host: &str) -> Result<Self, VaultError> {
        Self::parse(&format!("provider.{id}@{host}"))
    }

    /// The full entry name.
    pub fn as_str(&self) -> &str {
        &self.full
    }
    /// The kind (`provider`, `engine`, `integration`).
    pub fn kind(&self) -> &str {
        &self.full[..self.kind_end]
    }
    /// The id between kind and host.
    pub fn id(&self) -> &str {
        &self.full[self.kind_end + 1..self.at]
    }
    /// The host the secret is bound to.
    pub fn host(&self) -> &str {
        &self.full[self.at + 1..]
    }
}

/// Storage for secret values.
pub trait SecretBackend: Send + Sync {
    /// Store or replace a value.
    fn set(&self, name: &SecretName, value: &SecretString) -> Result<(), VaultError>;
    /// Fetch a value, `None` when absent.
    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, VaultError>;
    /// Delete a value; absent is not an error.
    fn delete(&self, name: &SecretName) -> Result<(), VaultError>;
    /// Whether a value exists.
    fn exists(&self, name: &SecretName) -> Result<bool, VaultError> {
        Ok(self.get(name)?.is_some())
    }
}

/// Windows Credential Manager (via keyring). Service is the brand name.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringBackend;

impl KeyringBackend {
    fn entry(name: &SecretName) -> Result<keyring::Entry, VaultError> {
        keyring::Entry::new(launcher_core::brand::APP_NAME, name.as_str())
            .map_err(|e| VaultError::Backend(e.to_string()))
    }
}

impl SecretBackend for KeyringBackend {
    fn set(&self, name: &SecretName, value: &SecretString) -> Result<(), VaultError> {
        Self::entry(name)?
            .set_password(value.expose_secret())
            .map_err(|e| VaultError::Backend(e.to_string()))
    }
    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, VaultError> {
        match Self::entry(name)?.get_password() {
            Ok(v) => Ok(Some(SecretString::from(v))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(VaultError::Backend(e.to_string())),
        }
    }
    fn delete(&self, name: &SecretName) -> Result<(), VaultError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(VaultError::Backend(e.to_string())),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// In-memory backend for tests.
#[derive(Default)]
pub struct MemoryBackend(Mutex<HashMap<String, String>>);

impl SecretBackend for MemoryBackend {
    fn set(&self, name: &SecretName, value: &SecretString) -> Result<(), VaultError> {
        lock(&self.0).insert(name.as_str().to_owned(), value.expose_secret().to_owned());
        Ok(())
    }
    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, VaultError> {
        Ok(lock(&self.0)
            .get(name.as_str())
            .map(|v| SecretString::from(v.clone())))
    }
    fn delete(&self, name: &SecretName) -> Result<(), VaultError> {
        lock(&self.0).remove(name.as_str());
        Ok(())
    }
}

/// Non-secret metadata for one entry. Never holds the value or last4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMeta {
    /// Entry name.
    pub name: String,
    /// Bound host.
    pub host: String,
    /// Unix seconds when stored.
    pub added_at: i64,
    /// Unix seconds when last used.
    pub last_used: Option<i64>,
}

/// Persistence for [`SecretMeta`]; the shell backs it with the `secret_meta` table.
pub trait MetaStore: Send + Sync {
    /// Insert or replace.
    fn put(&self, meta: SecretMeta) -> Result<(), VaultError>;
    /// Fetch one.
    fn get(&self, name: &str) -> Result<Option<SecretMeta>, VaultError>;
    /// Remove one; absent is fine.
    fn remove(&self, name: &str) -> Result<(), VaultError>;
    /// Set `last_used` in place under the store's own lock; no-op for a missing name.
    fn touch(&self, name: &str, last_used: i64) -> Result<(), VaultError>;
    /// All entries.
    fn list(&self) -> Result<Vec<SecretMeta>, VaultError>;
}

/// In-memory [`MetaStore`].
#[derive(Default)]
pub struct MemoryMeta(Mutex<HashMap<String, SecretMeta>>);

impl MetaStore for MemoryMeta {
    fn put(&self, meta: SecretMeta) -> Result<(), VaultError> {
        lock(&self.0).insert(meta.name.clone(), meta);
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Option<SecretMeta>, VaultError> {
        Ok(lock(&self.0).get(name).cloned())
    }
    fn remove(&self, name: &str) -> Result<(), VaultError> {
        lock(&self.0).remove(name);
        Ok(())
    }
    fn touch(&self, name: &str, last_used: i64) -> Result<(), VaultError> {
        if let Some(m) = lock(&self.0).get_mut(name) {
            m.last_used = Some(last_used);
        }
        Ok(())
    }
    fn list(&self) -> Result<Vec<SecretMeta>, VaultError> {
        let mut v: Vec<_> = lock(&self.0).values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }
}

/// What the UI may know about a secret. No value field exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SecretInfo {
    /// Entry name.
    pub name: String,
    /// Whether a value is stored.
    pub present: bool,
    /// Last four characters, only for values of 16 or more characters.
    pub last4: Option<String>,
    /// Bound host.
    pub host: String,
    /// Unix seconds when stored.
    pub added_at: Option<i64>,
    /// Unix seconds when last used.
    pub last_used: Option<i64>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Write-only secret vault.
pub struct SecretVault {
    backend: Arc<dyn SecretBackend>,
    meta: Arc<dyn MetaStore>,
    /// Unflushed `last_used` times by entry name.
    pending: Arc<Mutex<HashMap<String, i64>>>,
    /// Whether a background flush is already scheduled.
    scheduled: Arc<AtomicBool>,
}

impl SecretVault {
    /// Build a vault over a backend and a metadata store.
    pub fn new(backend: Arc<dyn SecretBackend>, meta: Arc<dyn MetaStore>) -> Self {
        Self {
            backend,
            meta,
            pending: Arc::default(),
            scheduled: Arc::default(),
        }
    }

    /// Store a secret bound to the host in its name. Rejects empty or multi-line values.
    pub fn set(&self, name: &SecretName, value: SecretString) -> Result<(), VaultError> {
        let v = value.expose_secret().trim();
        if v.is_empty() || v.contains(['\n', '\r']) {
            return Err(VaultError::InvalidValue);
        }
        let value = SecretString::from(v.to_owned());
        self.backend.set(name, &value)?;
        lock(&self.pending).remove(name.as_str());
        self.meta.put(SecretMeta {
            name: name.as_str().to_owned(),
            host: name.host().to_owned(),
            added_at: now(),
            last_used: None,
        })
    }

    /// Delete the value and its metadata.
    pub fn delete(&self, name: &SecretName) -> Result<(), VaultError> {
        self.backend.delete(name)?;
        lock(&self.pending).remove(name.as_str());
        self.meta.remove(name.as_str())
    }

    /// Describe one secret without exposing it.
    pub fn info(&self, name: &SecretName) -> Result<SecretInfo, VaultError> {
        let value = self.backend.get(name)?;
        let m = self.meta.get(name.as_str())?;
        let last4 = value.as_ref().and_then(|v| {
            let chars: Vec<char> = v.expose_secret().chars().collect();
            (chars.len() >= 16).then(|| chars[chars.len() - 4..].iter().collect())
        });
        Ok(SecretInfo {
            name: name.as_str().to_owned(),
            present: value.is_some(),
            last4,
            host: name.host().to_owned(),
            added_at: m.as_ref().map(|m| m.added_at),
            last_used: lock(&self.pending)
                .get(name.as_str())
                .copied()
                .or_else(|| m.and_then(|m| m.last_used)),
        })
    }

    /// Describe every secret with metadata.
    pub fn list(&self) -> Result<Vec<SecretInfo>, VaultError> {
        self.meta
            .list()?
            .iter()
            .map(|m| self.info(&SecretName::parse(&m.name)?))
            .collect()
    }

    /// Run `f` with the secret and record the use (tests only; no public read path).
    #[cfg(test)]
    pub(crate) fn with_secret<R>(
        &self,
        name: &SecretName,
        f: impl FnOnce(&SecretString) -> R,
    ) -> Result<R, VaultError> {
        let v = self.take(name)?.ok_or(VaultError::NotFound)?;
        Ok(f(&v))
    }

    /// Crate-internal read for key sources; records the use in memory and persists it
    /// later, coalesced, on a background thread. Blocking: call via `spawn_blocking`.
    pub(crate) fn take(&self, name: &SecretName) -> Result<Option<SecretString>, VaultError> {
        let v = self.backend.get(name)?;
        if v.is_some() {
            lock(&self.pending).insert(name.as_str().to_owned(), now());
            if !self.scheduled.swap(true, Ordering::AcqRel) {
                let (meta, pending, scheduled) = (
                    Arc::clone(&self.meta),
                    Arc::clone(&self.pending),
                    Arc::clone(&self.scheduled),
                );
                std::thread::spawn(move || {
                    std::thread::sleep(FLUSH_DELAY);
                    scheduled.store(false, Ordering::Release);
                    if flush_pending(&*meta, &pending).is_err() {
                        tracing::warn!("secret last_used flush failed");
                    }
                });
            }
        }
        Ok(v)
    }

    /// Persist pending `last_used` updates now (call on shutdown).
    pub fn flush(&self) -> Result<(), VaultError> {
        flush_pending(&*self.meta, &self.pending)
    }
}

/// How long `last_used` updates are held in memory before one coalesced write.
const FLUSH_DELAY: Duration = Duration::from_secs(5);

fn flush_pending(
    meta: &dyn MetaStore,
    pending: &Mutex<HashMap<String, i64>>,
) -> Result<(), VaultError> {
    let drained: Vec<_> = lock(pending).drain().collect();
    for (name, at) in drained {
        meta.touch(&name, at)?;
    }
    Ok(())
}

/// [`KeySource`] backed by the vault: `provider.<id>@<key_host>`.
#[derive(Clone)]
pub struct VaultKeys(pub Arc<SecretVault>);

impl KeySource for VaultKeys {
    fn key_for(&self, cfg: &ProviderConfig) -> Result<Option<SecretString>, ProviderError> {
        let Some(bound) = cfg.key_host.as_deref() else {
            return Ok(None);
        };
        let actual = cfg.base_url.host_str().unwrap_or_default();
        if !bound.eq_ignore_ascii_case(actual) {
            return Err(ProviderError::HostMismatch {
                bound: bound.to_owned(),
                actual: actual.to_owned(),
            });
        }
        let name = SecretName::provider(&cfg.id, bound)?;
        self.0
            .take(&name)?
            .map(Some)
            .ok_or_else(|| ProviderError::NoKey {
                host: bound.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_stores_trimmed_and_reads_back() {
        let v = SecretVault::new(
            Arc::new(MemoryBackend::default()),
            Arc::new(MemoryMeta::default()),
        );
        let name = SecretName::parse("provider.x@x.test").unwrap();
        v.set(
            &name,
            SecretString::from("  sk-fake-0123456789abcdef \t".to_owned()),
        )
        .unwrap();
        let got = v
            .with_secret(&name, |s| s.expose_secret().to_owned())
            .unwrap();
        assert_eq!(got, "sk-fake-0123456789abcdef");
        assert!(v.info(&name).unwrap().last_used.is_some());
        // take() must not touch the meta store synchronously; flush() persists it.
        let meta = v.meta.get(name.as_str()).unwrap().unwrap();
        assert_eq!(meta.last_used, None);
        v.flush().unwrap();
        let meta = v.meta.get(name.as_str()).unwrap().unwrap();
        assert!(meta.last_used.is_some());
        v.delete(&name).unwrap();
        assert!(matches!(
            v.with_secret(&name, |_| ()),
            Err(VaultError::NotFound)
        ));
    }

    #[test]
    fn flush_does_not_resurrect_deleted_or_revert_added_at() {
        let meta = MemoryMeta::default();
        let pending = Mutex::new(HashMap::from([("provider.x@x.test".to_owned(), 9)]));
        // Deleted after the pending insert: nothing comes back.
        flush_pending(&meta, &pending).unwrap();
        assert!(meta.list().unwrap().is_empty());
        // Re-set after take: added_at survives, last_used updates.
        meta.put(SecretMeta {
            name: "provider.x@x.test".into(),
            host: "x.test".into(),
            added_at: 77,
            last_used: None,
        })
        .unwrap();
        lock(&pending).insert("provider.x@x.test".into(), 9);
        flush_pending(&meta, &pending).unwrap();
        let m = meta.get("provider.x@x.test").unwrap().unwrap();
        assert_eq!((m.added_at, m.last_used), (77, Some(9)));
    }
}
