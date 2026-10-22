//! ADR-011 regression guards not covered by tests/redact.rs: JSON/URL masking, detector
//! false positives, secret-free config TOML and secret-free store schema.

use launcher_core::config::Config;
use launcher_core::detect::looks_secret;
use launcher_core::redact::Redactor;
use launcher_core::store::Store;

const TAIL: &str = concat!("aB3d", "E5gH7jK9mN1pQ3sT5vW7yZ9bD1fG3hJ5kL7m");

#[test]
fn masks_inside_json_and_urls() {
    let r = Redactor::new();
    r.register(b"fake-registered-value-123");
    for s in [
        format!(r#"{{"api_key":"sk-{TAIL}","x":1}}"#),
        format!(r#"{{"authorization":"Bearer {TAIL}"}}"#),
        format!("https://example.test/v1?key=sk-ant-api03-{TAIL}&a=b"),
        r#"{"k":"fake-registered-value-123"}"#.to_string(),
        "https://example.test/?token=fake-registered-value-123".to_string(),
    ] {
        let out = r.redact(&s);
        assert!(out.contains("[REDACTED]"), "not masked: {s}");
        assert!(!out.contains(TAIL), "tail leaked: {out}");
        assert!(!out.contains("fake-registered-value-123"), "leaked: {out}");
    }
}

#[test]
fn looks_secret_flags_keys_not_prose() {
    for s in [
        format!("sk-{TAIL}"),
        format!("sk-ant-api03-{TAIL}"),
        format!("sk-or-v1-{TAIL}"),
        format!("Authorization: Bearer {TAIL}"),
        TAIL.to_string(),
    ] {
        assert!(looks_secret(&s).is_some(), "missed: {s}");
    }
    for s in [
        "open the quarterly report and email it to Sam",
        r"C:\Users\someone\Documents\project-notes\risk-assessment-final.docx",
        "convert 100 usd to eur",
        "https://example.test/docs/getting-started",
    ] {
        assert!(looks_secret(s).is_none(), "false positive: {s}");
    }
}

#[test]
fn config_toml_carries_no_secret() {
    let mut cfg = Config::default();
    for e in cfg.providers.entries.values_mut() {
        e.enabled = true;
        e.key_host = Some("example.test".into());
    }
    for e in cfg.decision.engines.values_mut() {
        e.enabled = true;
        e.key_host = Some("example.test".into());
    }
    let text = toml::to_string(&cfg).unwrap();
    assert!(looks_secret(&text).is_none(), "key-like value in config");
    let bad = [
        "key", "api_key", "apikey", "token", "secret", "password", "bearer",
    ];
    for line in text.lines() {
        if let Some((name, _)) = line.split_once('=') {
            let name = name.trim().trim_matches('"').to_ascii_lowercase();
            assert!(
                !bad.contains(&name.as_str()),
                "secret-capable field: {name}"
            );
            assert!(
                !name.ends_with("_key") && !name.ends_with("_secret") && !name.ends_with("_token"),
                "secret-capable field: {name}"
            );
        }
    }
}

#[test]
fn store_schema_has_no_secret_column() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let (store, _) = Store::open(&path).unwrap();
    store.flush();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut q = conn
        .prepare("SELECT m.name, p.name FROM sqlite_master m, pragma_table_info(m.name) p WHERE m.type = 'table'")
        .unwrap();
    let cols: Vec<(String, String)> = q
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(!cols.is_empty());
    for (t, c) in cols {
        for bad in [
            "key_value",
            "secret",
            "token",
            "password",
            "api_key",
            "value",
        ] {
            assert!(c != bad, "{t}.{c} could hold a secret");
        }
        assert!(!c.contains("secret") || t == "secret_meta", "{t}.{c}");
    }
}
