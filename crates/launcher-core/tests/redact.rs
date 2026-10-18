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
