use std::borrow::Cow;

use launcher_core::redact::Redactor;

fn masked(r: &Redactor, s: &str) {
    let out = r.redact(s);
    assert!(out.contains("[REDACTED]"), "not masked: {s}");
}

#[test]
fn provider_samples_masked() {
    let r = Redactor::new();
    let tail = "aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bD1fG3hJ5kL7m";
    for s in [
        format!("sk-{tail}"),
        format!("key sk-ant-api03-{tail} end"),
        format!("ghp_{}", &tail[..36]),
        "AKIAABCDEFGHIJKLMNOP".to_string(),
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijklmnop".to_string(),
        format!("Authorization: Bearer {tail}"),
    ] {
        masked(&r, &s);
    }
    assert!(!r.redact(&format!("sk-ant-api03-{tail}")).contains(tail));
}

#[test]
fn literal_masked_repeated_and_unregistered() {
    let r = Redactor::new();
    r.register(b"hunter2-hunter2");
    let out = r.redact("a hunter2-hunter2 b hunter2-hunter2 c");
    assert_eq!(out, "a [REDACTED] b [REDACTED] c");
    r.unregister_all();
    assert!(matches!(r.redact("a hunter2-hunter2"), Cow::Borrowed(_)));
}

#[test]
fn plain_text_borrowed_and_short_ignored() {
    let r = Redactor::new();
    r.register(b"short");
    let out = r.redact("a short plain line");
    assert!(matches!(out, Cow::Borrowed(_)));
    assert_eq!(out, "a short plain line");
}

#[test]
fn debug_hides_secret() {
    let r = Redactor::new();
    r.register(b"super-secret-value");
    assert!(!format!("{r:?}").contains("super-secret"));
}

const BODY: &str = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj";

#[test]
fn pem_block_body_masked() {
    let r = Redactor::new();
    let t = format!("a -----BEGIN PRIVATE KEY-----\n{BODY}\n-----END PRIVATE KEY----- z");
    assert_eq!(r.redact(&t), "a [REDACTED] z");
}

#[test]
fn pem_without_end_masks_to_end() {
    let r = Redactor::new();
    let t = format!("x -----BEGIN RSA PRIVATE KEY-----\n{BODY}\nmore");
    assert_eq!(r.redact(&t), "x [REDACTED]");
}

#[test]
fn literal_overlapping_key_pattern_fully_masked() {
    let r = Redactor::new();
    let tail = "aB3dE5gH7jK9mN1pQ3sT5vW7yZ9";
    r.register(format!("{tail}-extra").as_bytes());
    let t = format!("k sk-ant-api03-{tail}-extra-more done");
    let out = r.redact(&t);
    assert_eq!(out, "k [REDACTED] done");
}

#[test]
fn adjacent_secrets_merge() {
    let r = Redactor::new();
    r.register(b"hunter2-hunter2");
    r.register(b"other-secret-1");
    assert_eq!(r.redact("hunter2-hunter2other-secret-1!"), "[REDACTED]!");
}

#[test]
fn multibyte_text_stays_valid() {
    let r = Redactor::new();
    r.register("pässwörd-ключ".as_bytes());
    assert_eq!(r.redact("é日本 pässwörd-ключ 😀"), "é日本 [REDACTED] 😀");
}
