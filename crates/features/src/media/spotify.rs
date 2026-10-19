//! Optional Spotify connection (ADR-010): OAuth PKCE with the user's own client id, a
//! one-shot loopback callback, and the few Web API calls the media feature uses.
//!
//! The refresh token lives only in the vault (`integration.spotify.refresh@accounts.spotify.com`);
//! the access token lives in memory with its expiry. All network happens in `execute()`
//! paths and the connect command, never in `query()`.

use arc_swap::ArcSwap;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use launcher_core::Config;
use providers::http::authed_client;
use providers::secret::{SecretName, SecretVault};
use reqwest::Method;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// Host the refresh token is bound to.
pub const ACCOUNTS_HOST: &str = "accounts.spotify.com";
/// Host the YouTube key is bound to.
pub const YOUTUBE_HOST: &str = "www.googleapis.com";
const ACCOUNTS_BASE: &str = "https://accounts.spotify.com";
const API_BASE: &str = "https://api.spotify.com/v1";
/// Request timeout (BSD External API table).
const TIMEOUT: Duration = Duration::from_secs(5);
/// How long the browser has to come back.
const CONNECT_WAIT: Duration = Duration::from_secs(180);
const MAX_BODY: usize = 1 << 20;
/// Least scopes for the actions below (docs checked 2026-10).
const SCOPES: &str = "user-modify-playback-state user-read-playback-state \
user-read-currently-playing user-library-modify playlist-read-private";

/// Spotify failures. Messages never carry tokens.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum SpotifyError {
    /// The user denied access or never came back.
    #[error("Spotify sign-in was cancelled")]
    AuthCancelled,
    /// The loopback port is taken.
    #[error("Port {0} is in use - close the app using it and retry")]
    PortBusy(u16),
    /// Transport failure or timeout.
    #[error("Could not reach Spotify")]
    Network,
    /// No Client ID saved.
    #[error("Save your Spotify Client ID first")]
    NoClientId,
    /// No stored refresh token.
    #[error("Spotify is not connected")]
    NotConnected,
    /// Spotify has no device to play on.
    #[error("No active Spotify device - start playing in Spotify once")]
    NoActiveDevice,
    /// Premium-only action on a free account.
    #[error("Spotify Premium is required for this action")]
    PremiumRequired,
    /// Nothing is playing.
    #[error("Nothing is playing on Spotify")]
    NothingPlaying,
    /// Any other non-success status.
    #[error("Spotify returned status {0}")]
    Api(u16),
    /// The secret store failed.
    #[error("Could not use the secret store")]
    Vault,
    /// Unexpected response or local failure.
    #[error("{0}")]
    Other(&'static str),
}

impl SpotifyError {
    /// Stable code for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthCancelled => "auth_cancelled",
            Self::PortBusy(_) => "port_busy",
            Self::Network => "network",
            Self::NoClientId => "no_client_id",
            _ => "spotify",
        }
    }
}

/// A random code verifier: 64 random bytes, 86 url-safe characters (RFC 7636: 43 to 128).
pub fn new_verifier() -> String {
    let mut b = [0u8; 64];
    rand::fill(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

/// S256 code challenge for `verifier`.
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// What a request line on the loopback listener means.
#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    /// Authorization code with the matching state.
    Code(String),
    /// The user denied, or Spotify reported an error, with the matching state.
    Denied,
    /// Not our callback (other path, wrong state, malformed).
    Ignore,
}

/// Parses `GET /callback?...` against the expected `state`.
pub fn parse_callback(request_line: &str, state: &str) -> Callback {
    let mut it = request_line.split_whitespace();
    let (Some("GET"), Some(target)) = (it.next(), it.next()) else {
        return Callback::Ignore;
    };
    let Ok(u) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Callback::Ignore;
    };
    if u.path() != "/callback" {
        return Callback::Ignore;
    }
    let get = |k: &str| {
        u.query_pairs()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.into_owned())
    };
    if get("state").as_deref() != Some(state) {
        return Callback::Ignore;
    }
    match (get("code"), get("error")) {
        (_, Some(_)) => Callback::Denied,
        (Some(c), None) if !c.is_empty() => Callback::Code(c),
        _ => Callback::Ignore,
    }
}

const PAGE: &str = "<!doctype html><meta charset=utf-8><title>Connected</title>\
<body style=\"font-family:sans-serif\"><p>You can close this tab and return to the launcher.</p>";
const PAGE_DENIED: &str = "<!doctype html><meta charset=utf-8><title>Cancelled</title>\
<body style=\"font-family:sans-serif\"><p>Sign-in cancelled. You can close this tab.</p>";

async fn reply(s: &mut tokio::net::TcpStream, status: &str, body: &str) {
    let r = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // Best effort: the browser may have gone away.
    let _ = s.write_all(r.as_bytes()).await;
    let _ = s.shutdown().await;
}

/// Waits on `listener` for our callback and returns the code. One-shot; other requests get 404.
pub async fn wait_callback(
    listener: &TcpListener,
    state: &str,
    wait: Duration,
) -> Result<String, SpotifyError> {
    let run = async {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                // A failing accept (e.g. out of handles) must not spin the CPU.
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            };
            let mut buf = vec![0u8; 8192];
            let n = tokio::time::timeout(Duration::from_secs(5), s.read(&mut buf))
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let line = head.lines().next().unwrap_or_default();
            match parse_callback(line, state) {
                Callback::Code(c) => {
                    reply(&mut s, "200 OK", PAGE).await;
                    return Ok(c);
                }
                Callback::Denied => {
                    reply(&mut s, "200 OK", PAGE_DENIED).await;
                    return Err(SpotifyError::AuthCancelled);
                }
                Callback::Ignore => reply(&mut s, "404 Not Found", "").await,
            }
        }
    };
    tokio::time::timeout(wait, run)
        .await
        .unwrap_or(Err(SpotifyError::AuthCancelled))
}

/// Reads a body, giving up past `max` bytes.
pub(crate) async fn read_capped(mut r: reqwest::Response, max: usize) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    while let Some(c) = r.chunk().await.ok()? {
        if buf.len() + c.len() > max {
            return None;
        }
        buf.extend_from_slice(&c);
    }
    Some(buf)
}

#[derive(Deserialize)]
struct Tok {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

struct Access {
    token: SecretString,
    expires: Instant,
}

/// A track hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// `spotify:track:<id>`.
    pub uri: String,
    /// Title.
    pub name: String,
}

/// A device or playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// Device id or playlist URI.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Device type, or empty.
    pub detail: String,
}

/// How a play request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Played {
    /// Spotify is playing it.
    Api,
    /// Could not play via the API; open this link in the Spotify app instead.
    Fallback(String),
}

/// The Spotify connection. Cheap to share behind an `Arc`.
pub struct Spotify {
    vault: Arc<SecretVault>,
    config: Arc<ArcSwap<Config>>,
    http: reqwest::Client,
    accounts: String,
    api: String,
    connected: AtomicBool,
    token: tokio::sync::Mutex<Option<Access>>,
    /// Called after the connection is dropped because Spotify revoked the grant.
    on_lost: parking_lot::Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl std::fmt::Debug for Spotify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spotify")
            .field("connected", &self.is_connected())
            .finish_non_exhaustive()
    }
}

/// Marker for a 400 `invalid_grant` token response.
const INVALID_GRANT: &str = "Spotify rejected the saved sign-in - connect again";

fn net(_: reqwest::Error) -> SpotifyError {
    SpotifyError::Network
}

fn refresh_name() -> Result<SecretName, SpotifyError> {
    SecretName::parse(&format!("integration.spotify.refresh@{ACCOUNTS_HOST}"))
        .map_err(|_| SpotifyError::Vault)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, SpotifyError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| SpotifyError::Other("background task failed"))
}

impl Spotify {
    /// New, disconnected until [`Spotify::init`] finds a stored token.
    pub fn new(
        vault: Arc<SecretVault>,
        config: Arc<ArcSwap<Config>>,
    ) -> Result<Self, SpotifyError> {
        Ok(Self {
            vault,
            config,
            http: authed_client().map_err(|_| SpotifyError::Other("http client failed"))?,
            accounts: ACCOUNTS_BASE.into(),
            api: API_BASE.into(),
            connected: AtomicBool::new(false),
            token: tokio::sync::Mutex::new(None),
            on_lost: parking_lot::Mutex::new(None),
        })
    }

    /// Registers what to run when a refresh finds the grant revoked and the connection is dropped.
    pub fn on_lost(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.on_lost.lock() = Some(Arc::new(f));
    }

    #[cfg(test)]
    fn with_bases(mut self, base: &str) -> Self {
        self.accounts = base.to_owned();
        self.api = base.to_owned();
        self
    }

    /// Cheap flag for `query()`.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Reads whether a refresh token is stored (blocking vault call, off the runtime).
    pub async fn init(&self) {
        let v = Arc::clone(&self.vault);
        let present = blocking(move || {
            refresh_name()
                .ok()
                .and_then(|n| v.info(&n).ok())
                .is_some_and(|i| i.present)
        })
        .await
        .unwrap_or(false);
        self.connected.store(present, Ordering::Relaxed);
    }

    /// The YouTube key, only when `use_api` is on and a key is stored.
    pub async fn youtube_key(&self) -> Option<SecretString> {
        if !self.config.load().integrations.youtube.use_api {
            return None;
        }
        let v = Arc::clone(&self.vault);
        blocking(move || v.integration_secret("youtube", YOUTUBE_HOST))
            .await
            .ok()
            .flatten()
    }

    /// HTTP client (no redirects).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    fn redirect_uri(&self) -> (String, u16) {
        let port = self.config.load().integrations.spotify.redirect_port;
        (format!("http://127.0.0.1:{port}/callback"), port)
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Tok, SpotifyError> {
        let resp = self
            .http
            .post(format!("{}/api/token", self.accounts))
            .timeout(TIMEOUT)
            .form(form)
            .send()
            .await
            .map_err(net)?;
        let status = resp.status();
        let body = read_capped(resp, MAX_BODY)
            .await
            .ok_or(SpotifyError::Network)?;
        if !status.is_success() {
            if status.as_u16() == 400 && String::from_utf8_lossy(&body).contains("invalid_grant") {
                return Err(SpotifyError::Other(INVALID_GRANT));
            }
            return Err(SpotifyError::Api(status.as_u16()));
        }
        serde_json::from_slice(&body).map_err(|_| SpotifyError::Other("bad token response"))
    }

    async fn store_refresh(&self, token: String) -> Result<(), SpotifyError> {
        let v = Arc::clone(&self.vault);
        blocking(move || {
            v.set(&refresh_name()?, SecretString::from(token))
                .map_err(|_| SpotifyError::Vault)
        })
        .await?
    }

    fn keep(guard: &mut Option<Access>, t: &Tok) -> SecretString {
        let token = SecretString::from(t.access_token.clone());
        *guard = Some(Access {
            token: token.clone(),
            expires: Instant::now() + Duration::from_secs(t.expires_in),
        });
        token
    }

    /// Runs the whole sign-in: loopback listener, browser, code exchange, vault, profile name.
    /// `open` launches the authorize URL in the default browser (blocking).
    pub async fn connect(
        &self,
        open: impl FnOnce(String) -> Result<(), String> + Send + 'static,
    ) -> Result<String, SpotifyError> {
        let client_id = self
            .config
            .load()
            .integrations
            .spotify
            .client_id
            .trim()
            .to_owned();
        if client_id.is_empty() {
            return Err(SpotifyError::NoClientId);
        }
        let (redirect, port) = self.redirect_uri();
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                SpotifyError::PortBusy(port)
            } else {
                SpotifyError::Other("could not listen on the loopback port")
            }
        })?;
        let verifier = new_verifier();
        let mut sb = [0u8; 16];
        rand::fill(&mut sb);
        let state = URL_SAFE_NO_PAD.encode(sb);
        let mut auth = Url::parse(&format!("{}/authorize", self.accounts))
            .map_err(|_| SpotifyError::Other("bad authorize url"))?;
        auth.query_pairs_mut()
            .append_pair("client_id", &client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect)
            .append_pair("state", &state)
            .append_pair("scope", SCOPES)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge(&verifier));
        blocking(move || open(auth.into()))
            .await?
            .map_err(|_| SpotifyError::Other("could not open the browser"))?;
        let code = wait_callback(&listener, &state, CONNECT_WAIT).await?;
        drop(listener);
        let t = self
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &redirect),
                ("client_id", &client_id),
                ("code_verifier", &verifier),
            ])
            .await?;
        let refresh = t
            .refresh_token
            .clone()
            .ok_or(SpotifyError::Other("Spotify sent no refresh token"))?;
        self.store_refresh(refresh).await?;
        Self::keep(&mut *self.token.lock().await, &t);
        self.connected.store(true, Ordering::Relaxed);
        let name = self
            .call(Method::GET, "/me", &[], None)
            .await
            .ok()
            .and_then(|v| {
                v["display_name"]
                    .as_str()
                    .or(v["id"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Spotify user".into());
        Ok(name)
    }

    /// Deletes the refresh token and drops the access token.
    pub async fn disconnect(&self) -> Result<(), SpotifyError> {
        let v = Arc::clone(&self.vault);
        blocking(move || v.delete(&refresh_name()?).map_err(|_| SpotifyError::Vault)).await??;
        *self.token.lock().await = None;
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    async fn refresh(&self) -> Result<SecretString, SpotifyError> {
        let mut g = self.token.lock().await;
        let v = Arc::clone(&self.vault);
        let stored = blocking(move || v.integration_secret("spotify.refresh", ACCOUNTS_HOST))
            .await?
            .ok_or(SpotifyError::NotConnected)?;
        let client_id = self.config.load().integrations.spotify.client_id.clone();
        let t = match self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", stored.expose_secret()),
                ("client_id", &client_id),
            ])
            .await
        {
            Ok(t) => t,
            Err(SpotifyError::Other(INVALID_GRANT)) => {
                // Revoked or issued for another Client ID: forget it and tell the UI.
                let v = Arc::clone(&self.vault);
                blocking(move || v.delete(&refresh_name()?).map_err(|_| SpotifyError::Vault))
                    .await??;
                *g = None;
                self.connected.store(false, Ordering::Relaxed);
                let cb = self.on_lost.lock().clone();
                if let Some(cb) = cb {
                    cb();
                }
                return Err(SpotifyError::NotConnected);
            }
            Err(e) => return Err(e),
        };
        if let Some(r) = t.refresh_token.clone() {
            self.store_refresh(r).await?;
        }
        Ok(Self::keep(&mut g, &t))
    }

    async fn access(&self) -> Result<SecretString, SpotifyError> {
        if let Some(a) = &*self.token.lock().await
            && a.expires > Instant::now() + Duration::from_secs(30)
        {
            return Ok(a.token.clone());
        }
        self.refresh().await
    }

    /// One API call; refreshes once on 401. Returns the JSON body (`Null` when empty).
    async fn call(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value, SpotifyError> {
        for attempt in 0..2 {
            let token = self.access().await?;
            let mut rb = self
                .http
                .request(method.clone(), format!("{}{path}", self.api))
                .timeout(TIMEOUT)
                .bearer_auth(token.expose_secret())
                .query(query);
            if let Some(b) = &body {
                rb = rb.json(b);
            }
            let resp = rb.send().await.map_err(net)?;
            let status = resp.status();
            if status.as_u16() == 401 && attempt == 0 {
                self.refresh().await?;
                continue;
            }
            let bytes = read_capped(resp, MAX_BODY)
                .await
                .ok_or(SpotifyError::Network)?;
            let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if status.is_success() {
                return Ok(v);
            }
            let reason = v["error"]["reason"].as_str().unwrap_or_default();
            return Err(match (status.as_u16(), reason) {
                (_, "NO_ACTIVE_DEVICE") => SpotifyError::NoActiveDevice,
                (_, "PREMIUM_REQUIRED") => SpotifyError::PremiumRequired,
                (s, _) => SpotifyError::Api(s),
            });
        }
        Err(SpotifyError::Api(401))
    }

    async fn search_track(&self, q: &str) -> Result<Option<Track>, SpotifyError> {
        let v = self
            .call(
                Method::GET,
                "/search",
                &[("q", q), ("type", "track"), ("limit", "1")],
                None,
            )
            .await?;
        let t = &v["tracks"]["items"][0];
        Ok(match (t["uri"].as_str(), t["name"].as_str()) {
            (Some(uri), Some(name)) => Some(Track {
                uri: uri.into(),
                name: name.into(),
            }),
            _ => None,
        })
    }

    /// Plays the top track for `q`. No device or no Premium falls back to opening it in the app.
    pub async fn play_query(&self, q: &str) -> Result<Played, SpotifyError> {
        let Some(t) = self.search_track(q).await? else {
            return Ok(Played::Fallback(super::links::spotify(q)));
        };
        match self
            .call(
                Method::PUT,
                "/me/player/play",
                &[],
                Some(json!({ "uris": [t.uri] })),
            )
            .await
        {
            Ok(_) => Ok(Played::Api),
            Err(SpotifyError::NoActiveDevice | SpotifyError::PremiumRequired) => {
                Ok(Played::Fallback(t.uri))
            }
            Err(e) => Err(e),
        }
    }

    /// Plays an album, artist or playlist URI. Premium.
    pub async fn play_context(&self, uri: &str) -> Result<(), SpotifyError> {
        self.call(
            Method::PUT,
            "/me/player/play",
            &[],
            Some(json!({ "context_uri": uri })),
        )
        .await
        .map(drop)
    }

    /// Adds the top track for `q` to the queue. Premium.
    pub async fn queue(&self, q: &str) -> Result<(), SpotifyError> {
        let t = self
            .search_track(q)
            .await?
            .ok_or(SpotifyError::Other("No track found"))?;
        self.call(Method::POST, "/me/player/queue", &[("uri", &t.uri)], None)
            .await
            .map(drop)
    }

    /// Saves the current track to the library.
    pub async fn like(&self) -> Result<(), SpotifyError> {
        let v = self
            .call(Method::GET, "/me/player/currently-playing", &[], None)
            .await?;
        let uri = v["item"]["uri"]
            .as_str()
            .ok_or(SpotifyError::NothingPlaying)?;
        self.call(Method::PUT, "/me/library", &[("uris", uri)], None)
            .await
            .map(drop)
    }

    /// Connect devices.
    pub async fn devices(&self) -> Result<Vec<Named>, SpotifyError> {
        let v = self
            .call(Method::GET, "/me/player/devices", &[], None)
            .await?;
        Ok(v["devices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                Some(Named {
                    id: d["id"].as_str()?.into(),
                    name: d["name"].as_str()?.into(),
                    detail: d["type"].as_str().unwrap_or_default().into(),
                })
            })
            .collect())
    }

    /// Moves playback to a device and keeps playing. Premium.
    pub async fn transfer(&self, device_id: &str) -> Result<(), SpotifyError> {
        self.call(
            Method::PUT,
            "/me/player",
            &[],
            Some(json!({ "device_ids": [device_id], "play": true })),
        )
        .await
        .map(drop)
    }

    /// Your own playlists whose name contains `q` (the first 50 are searched), up to 10.
    pub async fn playlists(&self, q: &str) -> Result<Vec<Named>, SpotifyError> {
        let v = self
            .call(Method::GET, "/me/playlists", &[("limit", "50")], None)
            .await?;
        let q = q.to_lowercase();
        Ok(v["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let name = p["name"].as_str()?;
                let uri = p["uri"].as_str()?;
                name.to_lowercase().contains(&q).then(|| Named {
                    id: uri.into(),
                    name: name.into(),
                    detail: String::new(),
                })
            })
            .take(10)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::secret::{MemoryBackend, MemoryMeta};
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sp(base: &str) -> Spotify {
        let vault = Arc::new(SecretVault::new(
            Arc::new(MemoryBackend::default()),
            Arc::new(MemoryMeta::default()),
        ));
        let mut cfg = Config::default();
        cfg.integrations.spotify.client_id = "cid".into();
        Spotify::new(vault, Arc::new(ArcSwap::from_pointee(cfg)))
            .unwrap()
            .with_bases(base)
    }

    fn put_refresh(s: &Spotify, v: &str) {
        s.vault
            .set(&refresh_name().unwrap(), SecretString::from(v.to_owned()))
            .unwrap();
    }

    fn tok(access: &str, refresh: Option<&str>) -> ResponseTemplate {
        let mut b = json!({ "access_token": access, "expires_in": 3600, "token_type": "Bearer" });
        if let Some(r) = refresh {
            b["refresh_token"] = r.into();
        }
        ResponseTemplate::new(200).set_body_json(b)
    }

    #[test]
    fn pkce_rfc7636_vector() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let v = new_verifier();
        assert!((43..=128).contains(&v.len()));
        assert_ne!(v, new_verifier());
    }

    #[test]
    fn callback_cases() {
        let ok = "GET /callback?code=abc&state=st HTTP/1.1";
        assert_eq!(parse_callback(ok, "st"), Callback::Code("abc".into()));
        assert_eq!(parse_callback(ok, "other"), Callback::Ignore);
        assert_eq!(
            parse_callback("GET /callback?error=access_denied&state=st HTTP/1.1", "st"),
            Callback::Denied
        );
        assert_eq!(
            parse_callback("GET /favicon.ico HTTP/1.1", "st"),
            Callback::Ignore
        );
        assert_eq!(
            parse_callback("POST /callback?code=a&state=st HTTP/1.1", "st"),
            Callback::Ignore
        );
    }

    #[tokio::test]
    async fn listener_returns_code_then_times_out() {
        let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let c = tokio::spawn(async move {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            s.write_all(b"GET /callback?code=zz&state=s1 HTTP/1.1\r\n\r\n")
                .await
                .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            out
        });
        assert_eq!(
            wait_callback(&l, "s1", Duration::from_secs(5)).await,
            Ok("zz".into())
        );
        assert!(c.await.unwrap().contains("You can close this tab"));
        assert_eq!(
            wait_callback(&l, "s1", Duration::from_millis(50)).await,
            Err(SpotifyError::AuthCancelled)
        );
    }

    #[tokio::test]
    async fn token_exchange_sends_pkce_form() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code_verifier=ver"))
            .respond_with(tok("acc", Some("ref")))
            .expect(1)
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        let t = s
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("code_verifier", "ver"),
            ])
            .await
            .unwrap();
        assert_eq!(
            (t.access_token.as_str(), t.refresh_token.as_deref()),
            ("acc", Some("ref"))
        );
    }

    #[tokio::test]
    async fn invalid_grant_disconnects_and_notifies() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({ "error": "invalid_grant" })),
            )
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        put_refresh(&s, "old");
        let hits = Arc::new(AtomicBool::new(false));
        let h = hits.clone();
        s.on_lost(move || h.store(true, Ordering::Relaxed));
        let e = s.devices().await.unwrap_err();
        assert!(matches!(e, SpotifyError::NotConnected));
        assert!(hits.load(Ordering::Relaxed));
        assert!(
            s.vault
                .integration_secret("spotify.refresh", ACCOUNTS_HOST)
                .is_none()
        );
    }

    #[tokio::test]
    async fn refreshes_once_on_401_and_rotates() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .and(body_string_contains("refresh_token=old"))
            .respond_with(tok("stale", None))
            .up_to_n_times(1)
            .expect(1)
            .mount(&m)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .and(body_string_contains("refresh_token=old"))
            .respond_with(tok("fresh", Some("new")))
            .expect(1)
            .mount(&m)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/player/devices"))
            .and(header("authorization", "Bearer stale"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&m)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/player/devices"))
            .and(header("authorization", "Bearer fresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ "devices": [{ "id": "d1", "name": "PC", "type": "Computer" }] }),
            ))
            .expect(1)
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        put_refresh(&s, "old");
        let d = s.devices().await.unwrap();
        assert_eq!((d[0].id.as_str(), d[0].name.as_str()), ("d1", "PC"));
        let rotated = s
            .vault
            .integration_secret("spotify.refresh", ACCOUNTS_HOST)
            .unwrap();
        assert_eq!(rotated.expose_secret(), "new");
    }

    #[tokio::test]
    async fn persistent_401_stops_after_one_refresh() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(tok("a", None))
            .expect(2)
            .mount(&m)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/player/devices"))
            .respond_with(ResponseTemplate::new(401))
            .expect(2)
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        put_refresh(&s, "r");
        assert_eq!(s.devices().await.unwrap_err(), SpotifyError::Api(401));
    }

    #[tokio::test]
    async fn no_active_device_falls_back_to_track_link() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(tok("a", None))
            .mount(&m)
            .await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tracks": { "items": [{ "uri": "spotify:track:t1", "name": "Song" }] }
            })))
            .mount(&m)
            .await;
        Mock::given(method("PUT"))
            .and(path("/me/player/play"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(
                    json!({ "error": { "status": 404, "reason": "NO_ACTIVE_DEVICE" } }),
                ),
            )
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        put_refresh(&s, "r");
        assert_eq!(
            s.play_query("song").await.unwrap(),
            Played::Fallback("spotify:track:t1".into())
        );
    }

    #[tokio::test]
    async fn playlist_search_maps_and_filters() {
        let m = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(tok("a", None))
            .mount(&m)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/playlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [
                { "name": "Gym Mix", "uri": "spotify:playlist:p1" },
                { "name": "Focus", "uri": "spotify:playlist:p2" }
            ]})))
            .mount(&m)
            .await;
        let s = sp(&m.uri());
        put_refresh(&s, "r");
        let got = s.playlists("gym").await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].id.as_str(), got[0].name.as_str()),
            ("spotify:playlist:p1", "Gym Mix")
        );
    }
}
