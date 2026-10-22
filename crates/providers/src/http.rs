//! Host-bound HTTP clients (ADR-011): no redirects with credentials, https only.

use crate::secret::{SecretName, SecretVault};
use crate::{KeySource, ProviderConfig, ProviderError};
use secrecy::SecretString;
use std::sync::Arc;
use std::time::Duration;
use url::{Host, Url};

/// Connect timeout for every client.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

fn build(policy: reqwest::redirect::Policy) -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .redirect(policy)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|_| ProviderError::Decode("http client build failed"))
}

/// Client for requests that carry a key: never follows redirects.
pub fn authed_client() -> Result<reqwest::Client, ProviderError> {
    build(reqwest::redirect::Policy::none())
}

/// Client for unauthenticated calls: follows at most 5 redirects.
pub fn plain_client() -> Result<reqwest::Client, ProviderError> {
    build(reqwest::redirect::Policy::limited(5))
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Allow only `bound_host`, over https (http only for loopback).
pub fn check_url(url: &Url, bound_host: &str) -> Result<(), ProviderError> {
    let host = url.host_str().unwrap_or_default();
    if !host.eq_ignore_ascii_case(bound_host) {
        return Err(ProviderError::HostMismatch {
            bound: bound_host.to_owned(),
            actual: host.to_owned(),
        });
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(url) => Ok(()),
        _ => Err(ProviderError::InsecureUrl {
            host: host.to_owned(),
        }),
    }
}

/// Call when a provider's base URL changes. If the host changed, the key bound
/// to the old host is deleted (ADR-011 rule 6). Returns true when the user
/// must enter a key again.
pub fn on_base_url_change(
    vault: &SecretVault,
    old_cfg: &ProviderConfig,
    new_cfg: &ProviderConfig,
) -> Result<bool, ProviderError> {
    if old_cfg.base_url.host_str() == new_cfg.base_url.host_str() {
        return Ok(false);
    }
    let Some(old_host) = old_cfg.key_host.as_deref() else {
        return Ok(false);
    };
    vault.delete(&SecretName::provider(&old_cfg.id, old_host)?)?;
    Ok(true)
}

/// Read a response body as a stream, failing with `Decode` once it exceeds `max` bytes.
pub(crate) async fn read_capped(
    mut resp: reqwest::Response,
    max: usize,
) -> Result<Vec<u8>, ProviderError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(net_err)? {
        if buf.len() + chunk.len() > max {
            return Err(ProviderError::Decode("response too large"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Map a transport error to its kind only, never the URL or body.
pub(crate) fn net_err(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout { phase: "response" }
    } else {
        ProviderError::Network(
            if e.is_connect() {
                "connect failed"
            } else {
                "request failed"
            }
            .into(),
        )
    }
}

/// Fail with `Status` (code only, no body) on a non-2xx response.
pub(crate) fn check_status(resp: &reqwest::Response) -> Result<(), ProviderError> {
    let s = resp.status();
    if s.is_success() {
        Ok(())
    } else {
        Err(ProviderError::Status { code: s.as_u16() })
    }
}

/// Fetch the key (if any) and validate host binding and scheme. The caller sets
/// its own sensitive header from the returned key.
///
/// The lookup runs on the blocking pool (Credential Manager is synchronous); the key
/// crosses that boundary only as a `SecretString`.
pub(crate) async fn authorize(
    cfg: &ProviderConfig,
    keys: &Arc<dyn KeySource>,
) -> Result<Option<SecretString>, ProviderError> {
    let (c, k) = (cfg.clone(), Arc::clone(keys));
    let found = tokio::task::spawn_blocking(move || k.key_for(&c))
        .await
        .map_err(|_| ProviderError::Decode("key lookup task failed"))??;
    let Some(key) = found else {
        return Ok(None);
    };
    let host = cfg.base_url.host_str().unwrap_or_default();
    match &cfg.key_host {
        Some(bound) => check_url(&cfg.base_url, bound)?,
        None => check_url(&cfg.base_url, host)?,
    }
    Ok(Some(key))
}

/// A sensitive header value built from `raw`.
pub(crate) fn sensitive(raw: &str) -> Result<reqwest::header::HeaderValue, ProviderError> {
    let mut v = reqwest::header::HeaderValue::from_str(raw)
        .map_err(|_| ProviderError::Decode("key not header-safe"))?;
    v.set_sensitive(true);
    Ok(v)
}
