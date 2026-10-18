//! Secret vault and host-bound HTTP tests (fake values only).

use providers::http::{authed_client, check_url, on_base_url_change};
use providers::secret::*;
use providers::{KeySource, ProviderConfig, ProviderError, Wire};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use url::Url;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAKE: &str = "sk-fake-0123456789abcdef";

fn vault() -> Arc<SecretVault> {
    Arc::new(SecretVault::new(
        Arc::new(MemoryBackend::default()),
        Arc::new(MemoryMeta::default()),
    ))
}
fn n(s: &str) -> SecretName {
    SecretName::parse(s).unwrap()
}
fn sec(s: &str) -> SecretString {
    SecretString::from(s.to_owned())
}
fn cfg(base: &str, key_host: Option<&str>) -> ProviderConfig {
    ProviderConfig {
        id: "openrouter".into(),
        wire: Wire::OpenAi,
        base_url: Url::parse(base).unwrap(),
        model: "m".into(),
        key_host: key_host.map(Into::into),
        supports_effort: false,
    }
}

#[test]
fn names_parse_and_reject() {
    let nm = n("integration.spotify.refresh@accounts.spotify.com");
    assert_eq!(
        (nm.kind(), nm.id(), nm.host()),
        ("integration", "spotify.refresh", "accounts.spotify.com")
    );
    for bad in [
        "",
        "provider@x.com",
        "provider.@x.com",
        "Provider.a@x.com",
        "provider.a@",
        "nope.a@x.com",
        "provider.a b@x.com",
        "provider.a@X.com",
    ] {
        assert!(SecretName::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn roundtrip_and_no_leak() {
    let v = vault();
    let name = n("provider.openrouter@openrouter.ai");
    v.set(&name, sec(FAKE)).unwrap();
    assert_eq!(
        v.with_secret(&name, |s| s.expose_secret().to_owned())
            .unwrap(),
        FAKE
    );
    let info = v.info(&name).unwrap();
    assert!(info.present && info.last_used.is_some() && info.added_at.is_some());
    assert_eq!(info.last4.as_deref(), Some("cdef"));
    let all = v.list().unwrap();
    assert_eq!(all.len(), 1);
    for s in [
        format!("{info:?}"),
        serde_json::to_string(&info).unwrap(),
        format!("{all:?}"),
        serde_json::to_string(&all).unwrap(),
    ] {
        assert!(!s.contains(FAKE) && !s.contains("0123456789"));
    }
    v.delete(&name).unwrap();
    assert!(!v.info(&name).unwrap().present);
    assert!(matches!(
        v.with_secret(&name, |_| ()),
        Err(VaultError::NotFound)
    ));
}

#[test]
fn last4_needs_16_chars_and_bad_values_rejected() {
    let v = vault();
    let name = n("engine.jev@api.typesafe.ai");
    v.set(&name, sec("123456789012345")).unwrap();
    assert_eq!(v.info(&name).unwrap().last4, None);
    for bad in ["", "   ", "a\nb", "a\r\nb"] {
        assert!(matches!(
            v.set(&name, sec(bad)),
            Err(VaultError::InvalidValue)
        ));
    }
}

#[test]
fn vault_keys_host_binding() {
    let v = vault();
    v.set(&n("provider.openrouter@openrouter.ai"), sec(FAKE))
        .unwrap();
    let keys = VaultKeys(v.clone());
    let ok = cfg("https://openrouter.ai/api/v1", Some("openrouter.ai"));
    assert_eq!(keys.key_for(&ok).unwrap().unwrap().expose_secret(), FAKE);
    let bad = cfg("https://evil.example/v1", Some("openrouter.ai"));
    assert!(matches!(
        keys.key_for(&bad),
        Err(ProviderError::HostMismatch { .. })
    ));
    assert!(
        keys.key_for(&cfg("http://localhost:11434/v1", None))
            .unwrap()
            .is_none()
    );
    let missing = cfg("https://x.test/v1", Some("x.test"));
    assert!(matches!(
        keys.key_for(&missing),
        Err(ProviderError::NoKey { .. })
    ));
}

#[test]
fn check_url_rules() {
    let u = |s: &str| Url::parse(s).unwrap();
    assert!(check_url(&u("https://api.openai.com/v1"), "api.openai.com").is_ok());
    assert!(matches!(
        check_url(&u("http://example.com/v1"), "example.com"),
        Err(ProviderError::InsecureUrl { .. })
    ));
    assert!(matches!(
        check_url(&u("https://other.com/v1"), "example.com"),
        Err(ProviderError::HostMismatch { .. })
    ));
    assert!(check_url(&u("http://localhost:11434/v1"), "localhost").is_ok());
    assert!(check_url(&u("http://127.0.0.1:1/v1"), "127.0.0.1").is_ok());
    assert!(check_url(&u("http://[::1]:1/v1"), "[::1]").is_ok());
}

#[tokio::test]
async fn authed_client_does_not_follow_redirects() {
    let b = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .mount(&b)
        .await;
    let a = MockServer::start().await;
    Mock::given(any())
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/steal", b.uri())),
        )
        .mount(&a)
        .await;
    let r = authed_client()
        .get(format!("{}/x", a.uri()))
        .header("authorization", "Bearer fake")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 302);
    assert!(b.received_requests().await.unwrap().is_empty());
}

#[test]
fn host_change_deletes_key() {
    let v = vault();
    let name = n("provider.openrouter@openrouter.ai");
    v.set(&name, sec(FAKE)).unwrap();
    let old = cfg("https://openrouter.ai/api/v1", Some("openrouter.ai"));
    let same = cfg("https://openrouter.ai/other", Some("openrouter.ai"));
    assert!(!on_base_url_change(&v, &old, &same).unwrap());
    assert!(v.info(&name).unwrap().present);
    let new = cfg("https://evil.example/v1", Some("openrouter.ai"));
    assert!(on_base_url_change(&v, &old, &new).unwrap());
    assert!(!v.info(&name).unwrap().present);
}

#[test]
#[ignore = "touches the real Windows Credential Manager"]
fn live_keyring_roundtrip() {
    let v = SecretVault::new(Arc::new(KeyringBackend), Arc::new(MemoryMeta::default()));
    let name = n("test.selftest@localhost");
    v.set(&name, sec(FAKE)).unwrap();
    let got = v.with_secret(&name, |s| s.expose_secret().to_owned());
    v.delete(&name).unwrap();
    assert_eq!(got.unwrap(), FAKE);
    assert!(!v.info(&name).unwrap().present);
}
