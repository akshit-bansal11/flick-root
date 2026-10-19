//! Integration settings (ADR-010, BSD section 3). Secrets are write-only through
//! `set_secret`; this module only reports `present` flags and non-secret config.
use super::blocking;
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::config::{Config, Webhook};
use providers::secret::{SecretName, SecretVault};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use std::sync::Arc;
use tauri::State;

const NOTION_HOST: &str = "api.notion.com";
const YOUTUBE_HOST: &str = "www.googleapis.com";
const SPOTIFY_HOST: &str = "accounts.spotify.com";

/// Host an `integration.<id>@...` secret must be bound to. The Spotify refresh token
/// (`spotify.refresh`) is deliberately absent: only the connect flow writes it.
pub(crate) fn bound_host(cfg: &Config, id: &str) -> Option<String> {
    match id {
        "notion" => Some(NOTION_HOST.into()),
        "youtube" => Some(YOUTUBE_HOST.into()),
        _ => {
            let n = id.strip_prefix("webhook.")?;
            let w = cfg.integrations.webhooks.iter().find(|w| w.name == n)?;
            Some(w.host.clone()).filter(|h| !h.is_empty())
        }
    }
}

fn present(vault: &SecretVault, name: &str) -> bool {
    SecretName::parse(name)
        .ok()
        .and_then(|n| vault.info(&n).ok())
        .is_some_and(|i| i.present)
}

/// Spotify as the UI sees it.
#[derive(Debug, Serialize)]
pub struct SpotifyView {
    /// Public PKCE client id.
    pub client_id: String,
    /// Loopback redirect URI to register in the user's Spotify app.
    pub redirect_uri: String,
    /// A refresh token is stored.
    pub connected: bool,
}

/// Webhook as the UI sees it; never the URL.
#[derive(Debug, Serialize)]
pub struct WebhookView {
    /// Display name (also the secret id).
    pub name: String,
    /// Enabled flag.
    pub enabled: bool,
    /// Host the URL secret is bound to.
    pub host: String,
    /// A URL is stored.
    pub url_present: bool,
}

/// Non-secret integration config plus `present` flags.
#[derive(Debug, Serialize)]
pub struct IntegrationsView {
    /// Spotify.
    pub spotify: SpotifyView,
    /// Use the YouTube API.
    pub youtube_use_api: bool,
    /// A YouTube key is stored.
    pub youtube_key_present: bool,
    /// Obsidian vault folder.
    pub obsidian_vault: String,
    /// Daily note folder, relative to the vault.
    pub obsidian_daily_folder: String,
    /// Daily note file name format.
    pub obsidian_daily_format: String,
    /// Markdown capture file.
    pub markdown_file: String,
    /// Notion database id.
    pub notion_database_id: String,
    /// A Notion token is stored.
    pub notion_token_present: bool,
    /// Webhooks.
    pub webhooks: Vec<WebhookView>,
}

fn view(vault: &SecretVault, cfg: &Config) -> IntegrationsView {
    let i = &cfg.integrations;
    IntegrationsView {
        spotify: SpotifyView {
            client_id: i.spotify.client_id.clone(),
            redirect_uri: format!("http://127.0.0.1:{}/callback", i.spotify.redirect_port),
            connected: present(
                vault,
                &format!("integration.spotify.refresh@{SPOTIFY_HOST}"),
            ),
        },
        youtube_use_api: i.youtube.use_api,
        youtube_key_present: present(vault, &format!("integration.youtube@{YOUTUBE_HOST}")),
        obsidian_vault: i.obsidian.vault.clone(),
        obsidian_daily_folder: i.obsidian.daily_folder.clone(),
        obsidian_daily_format: i.obsidian.daily_note_format.clone(),
        markdown_file: i.markdown_file.clone(),
        notion_database_id: i.notion.database_id.clone(),
        notion_token_present: present(vault, &format!("integration.notion@{NOTION_HOST}")),
        webhooks: i
            .webhooks
            .iter()
            .map(|w| WebhookView {
                name: w.name.clone(),
                enabled: w.enabled,
                host: w.host.clone(),
                url_present: present(vault, &format!("integration.webhook.{}@{}", w.name, w.host)),
            })
            .collect(),
    }
}

/// Reads integration settings; carries no secret value.
#[tauri::command]
pub async fn get_integrations(
    state: State<'_, Arc<AppState>>,
) -> Result<IntegrationsView, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || Ok(view(&s.vault, &s.config.load()))).await
}

/// Fields `set_integration` may change. Webhook URLs are not accepted here.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegrationPatch {
    /// Spotify client id; empty clears.
    pub client_id: Option<String>,
    /// YouTube `use_api`, or webhook enabled.
    pub enabled: Option<bool>,
    /// Obsidian vault folder.
    pub vault: Option<String>,
    /// Obsidian daily folder (vault-relative).
    pub daily_folder: Option<String>,
    /// Obsidian daily note format.
    pub daily_format: Option<String>,
    /// Markdown capture file.
    pub file: Option<String>,
    /// Notion database id.
    pub database_id: Option<String>,
    /// Webhook name.
    pub name: Option<String>,
    /// Webhook URL host (not the URL).
    pub host: Option<String>,
    /// Webhook new name (rename).
    pub new_name: Option<String>,
    /// Remove the webhook.
    pub remove: Option<bool>,
}

fn bad(code: &'static str, msg: &'static str) -> CommandError {
    CommandError::new(code, msg)
}

fn slug(s: &str, extra: &[char]) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(&c))
}

fn check_name(n: &str) -> Result<(), CommandError> {
    if n.len() <= 32 && slug(n, &['-', '_']) {
        Ok(())
    } else {
        Err(bad("invalid_name", "use 1 to 32 of a-z, 0-9, - and _"))
    }
}

fn check_host(h: &str) -> Result<(), CommandError> {
    if h.len() <= 253 && slug(h, &['.', '-']) && !h.ends_with('.') {
        Ok(())
    } else {
        Err(bad(
            "invalid_host",
            "enter the host only, for example hooks.example.com",
        ))
    }
}

fn invalid_path(m: &'static str) -> CommandError {
    bad("invalid_path", m)
}

fn check_dir(p: &str) -> Result<(), CommandError> {
    if p.is_empty() || Path::new(p).is_dir() {
        Ok(())
    } else {
        Err(invalid_path("that folder does not exist"))
    }
}

fn check_file(p: &str) -> Result<(), CommandError> {
    if p.is_empty() {
        return Ok(());
    }
    let path = Path::new(p);
    if path.file_name().is_none() || path.is_dir() {
        return Err(invalid_path("choose a file, not a folder"));
    }
    match path.parent() {
        Some(d) if d.as_os_str().is_empty() || d.is_dir() => Ok(()),
        _ => Err(invalid_path("the file's folder does not exist")),
    }
}

fn check_relative(p: &str) -> Result<(), CommandError> {
    if Path::new(p)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        Ok(())
    } else {
        Err(invalid_path("use a folder path inside the vault"))
    }
}

fn check_format(f: &str) -> Result<(), CommandError> {
    if !f.is_empty() && f.len() <= 64 && !f.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
    {
        Ok(())
    } else {
        Err(bad("invalid_format", "not a valid file name format"))
    }
}

fn check_database_id(id: &str) -> Result<(), CommandError> {
    let hex = id.chars().filter(|c| *c != '-').count();
    if id.is_empty() || (hex == 32 && id.chars().all(|c| c == '-' || c.is_ascii_hexdigit())) {
        Ok(())
    } else {
        Err(bad(
            "invalid_id",
            "a Notion database id is 32 hex characters",
        ))
    }
}

fn check_client_id(id: &str) -> Result<(), CommandError> {
    if id.is_empty() || (id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())) {
        Ok(())
    } else {
        Err(bad(
            "invalid_id",
            "a Spotify client id is letters and digits",
        ))
    }
}

fn drop_url(vault: &SecretVault, w: &Webhook) -> Result<(), CommandError> {
    if let Ok(n) = SecretName::parse(&format!("integration.webhook.{}@{}", w.name, w.host)) {
        vault
            .delete(&n)
            .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
    }
    Ok(())
}

fn webhook(
    vault: &SecretVault,
    cfg: &mut Config,
    patch: IntegrationPatch,
) -> Result<(), CommandError> {
    let name = patch.name.as_deref().unwrap_or_default().to_owned();
    check_name(&name)?;
    let list = &mut cfg.integrations.webhooks;
    let pos = list.iter().position(|w| w.name == name);
    if patch.remove == Some(true) {
        let at = pos.ok_or_else(|| bad("invalid_name", "no such webhook"))?;
        drop_url(vault, &list[at])?;
        list.remove(at);
        return Ok(());
    }
    let Some(at) = pos else {
        let host = patch.host.as_deref().unwrap_or_default();
        check_host(host)?;
        list.push(Webhook {
            name,
            enabled: patch.enabled.unwrap_or(false),
            host: host.to_owned(),
        });
        return Ok(());
    };
    // A changed name or host orphans the stored URL (its secret name changes): drop it.
    let old = list[at].clone();
    let mut w = old.clone();
    if let Some(n) = &patch.new_name {
        check_name(n)?;
        if n != &old.name && list.iter().any(|x| &x.name == n) {
            return Err(bad("invalid_name", "a webhook with that name exists"));
        }
        w.name = n.clone();
    }
    if let Some(h) = &patch.host {
        check_host(h)?;
        w.host = h.clone();
    }
    if let Some(e) = patch.enabled {
        w.enabled = e;
    }
    if (w.name != old.name || w.host != old.host) && present(vault, &secret_of(&old)) {
        drop_url(vault, &old)?;
    }
    list[at] = w;
    Ok(())
}

fn secret_of(w: &Webhook) -> String {
    format!("integration.webhook.{}@{}", w.name, w.host)
}

/// Applies `patch` to `target`. Validation happens before any change.
pub(crate) fn apply(
    vault: &SecretVault,
    cfg: &mut Config,
    target: &str,
    patch: IntegrationPatch,
) -> Result<(), CommandError> {
    let i = &mut cfg.integrations;
    match target {
        "spotify" => {
            if let Some(c) = patch.client_id {
                let c = c.trim().to_owned();
                check_client_id(&c)?;
                i.spotify.client_id = c;
            }
        }
        "youtube" => {
            if let Some(e) = patch.enabled {
                i.youtube.use_api = e;
            }
        }
        "obsidian" => {
            let vault_dir = patch.vault.as_deref().map(str::trim);
            let folder = patch.daily_folder.as_deref().map(str::trim);
            let format = patch.daily_format.as_deref().map(str::trim);
            vault_dir.map_or(Ok(()), check_dir)?;
            folder.map_or(Ok(()), check_relative)?;
            format.map_or(Ok(()), check_format)?;
            if let Some(v) = vault_dir {
                i.obsidian.vault = v.into();
            }
            if let Some(v) = folder {
                i.obsidian.daily_folder = v.into();
            }
            if let Some(v) = format {
                i.obsidian.daily_note_format = v.into();
            }
        }
        "markdown" => {
            if let Some(f) = patch.file.as_deref().map(str::trim) {
                check_file(f)?;
                i.markdown_file = f.into();
            }
        }
        "notion" => {
            if let Some(d) = patch.database_id.as_deref().map(str::trim) {
                check_database_id(d)?;
                i.notion.database_id = d.into();
            }
        }
        "webhook" => return webhook(vault, cfg, patch),
        _ => return Err(bad("unknown_target", "no such integration")),
    }
    Ok(())
}

/// Patches one integration, saves the config and returns the refreshed view.
#[tauri::command]
pub async fn set_integration(
    state: State<'_, Arc<AppState>>,
    target: String,
    patch: IntegrationPatch,
) -> Result<IntegrationsView, CommandError> {
    let s = Arc::clone(&state);
    blocking(move || {
        s.update_config(|cfg| apply(&s.vault, cfg, &target, patch))?;
        Ok(view(&s.vault, &s.config.load()))
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

    fn run(cfg: &mut Config, t: &str, p: IntegrationPatch) -> Result<(), CommandError> {
        apply(&vault(), cfg, t, p)
    }

    fn code(t: &str, p: IntegrationPatch) -> String {
        run(&mut Config::default(), t, p).unwrap_err().code
    }

    fn hook(name: &str, host: &str) -> IntegrationPatch {
        IntegrationPatch {
            name: Some(name.into()),
            host: Some(host.into()),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_bad_paths_ids_and_names() {
        let p = |v: &str| IntegrationPatch {
            vault: Some(v.into()),
            ..Default::default()
        };
        assert_eq!(
            code("obsidian", p("Z:/definitely/not/here")),
            "invalid_path"
        );
        let dir = IntegrationPatch {
            daily_folder: Some("../up".into()),
            ..Default::default()
        };
        assert_eq!(code("obsidian", dir), "invalid_path");
        let fmt = IntegrationPatch {
            daily_format: Some("a/b".into()),
            ..Default::default()
        };
        assert_eq!(code("obsidian", fmt), "invalid_format");
        let md = IntegrationPatch {
            file: Some("Z:/nope/dir/notes.md".into()),
            ..Default::default()
        };
        assert_eq!(code("markdown", md), "invalid_path");
        let nid = IntegrationPatch {
            database_id: Some("abc".into()),
            ..Default::default()
        };
        assert_eq!(code("notion", nid), "invalid_id");
        let sid = IntegrationPatch {
            client_id: Some("bad id!".into()),
            ..Default::default()
        };
        assert_eq!(code("spotify", sid), "invalid_id");
        assert_eq!(code("webhook", hook("Bad Name", "a.test")), "invalid_name");
        assert_eq!(
            code("webhook", hook("ok", "https://a.test/x")),
            "invalid_host"
        );
        assert_eq!(code("nope", IntegrationPatch::default()), "unknown_target");
    }

    #[test]
    fn accepts_good_values() {
        let dir = std::env::temp_dir();
        let d = dir.to_string_lossy().into_owned();
        let mut cfg = Config::default();
        let p = IntegrationPatch {
            vault: Some(d.clone()),
            daily_folder: Some("Daily/2026".into()),
            ..Default::default()
        };
        run(&mut cfg, "obsidian", p).unwrap();
        assert_eq!(cfg.integrations.obsidian.vault, d);
        let f = dir.join("inbox.md").to_string_lossy().into_owned();
        let p = IntegrationPatch {
            file: Some(f.clone()),
            ..Default::default()
        };
        run(&mut cfg, "markdown", p).unwrap();
        assert_eq!(cfg.integrations.markdown_file, f);
        let p = IntegrationPatch {
            database_id: Some("0123456789abcdef0123456789ABCDEF".into()),
            ..Default::default()
        };
        run(&mut cfg, "notion", p).unwrap();
    }

    #[test]
    fn webhook_lifecycle_drops_url_when_binding_changes() {
        let (v, mut cfg) = (vault(), Config::default());
        apply(&v, &mut cfg, "webhook", hook("a", "hooks.test")).unwrap();
        let n = SecretName::parse("integration.webhook.a@hooks.test").unwrap();
        v.set(&n, "https://hooks.test/secret-path".to_owned().into())
            .unwrap();
        assert_eq!(bound_host(&cfg, "webhook.a").as_deref(), Some("hooks.test"));
        let on = IntegrationPatch {
            name: Some("a".into()),
            enabled: Some(true),
            ..Default::default()
        };
        apply(&v, &mut cfg, "webhook", on).unwrap();
        assert!(v.info(&n).unwrap().present && cfg.integrations.webhooks[0].enabled);
        apply(&v, &mut cfg, "webhook", hook("a", "other.test")).unwrap();
        assert!(!v.info(&n).unwrap().present);
        let rn = IntegrationPatch {
            name: Some("a".into()),
            new_name: Some("b".into()),
            ..Default::default()
        };
        apply(&v, &mut cfg, "webhook", rn).unwrap();
        assert_eq!(cfg.integrations.webhooks[0].name, "b");
        let rm = IntegrationPatch {
            name: Some("b".into()),
            remove: Some(true),
            ..Default::default()
        };
        apply(&v, &mut cfg, "webhook", rm).unwrap();
        assert!(cfg.integrations.webhooks.is_empty());
    }

    #[test]
    fn view_carries_flags_not_values() {
        let v = vault();
        let n = SecretName::parse("integration.notion@api.notion.com").unwrap();
        v.set(&n, "secret_abcdefghijklmnop".to_owned().into())
            .unwrap();
        let out = serde_json::to_string(&view(&v, &Config::default())).unwrap();
        assert!(out.contains("\"notion_token_present\":true"));
        assert!(!out.contains("secret_abcdefghijklmnop"));
    }
}
