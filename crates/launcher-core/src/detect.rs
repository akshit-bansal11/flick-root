//! Secret detector used by Explain (block before sending) and the redactor.
//! Key patterns use `\b`, so a key glued to a letter or digit (`risk-assessment`) is ignored;
//! match ranges start at the key itself, which the redactor relies on.
//! Choice: 40-char hex strings (git SHAs) are not secrets: hex entropy is at most 4.0
//! and in practice below it, and a letters-and-digits mix is also required.

use regex::Regex;
use std::sync::LazyLock;

/// Kind of secret that was recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// Anthropic `sk-ant-` key.
    Anthropic,
    /// OpenAI-style `sk-` key.
    OpenAi,
    /// GitHub token.
    GitHub,
    /// AWS access key id.
    Aws,
    /// Slack token.
    Slack,
    /// Google API key.
    Google,
    /// `Bearer` authorization token.
    Bearer,
    /// JSON web token.
    Jwt,
    /// PEM private key header.
    PrivateKey,
    /// Long high-entropy token.
    Generic,
}

const PATTERNS: [(SecretKind, &str); 9] = [
    (SecretKind::Anthropic, r"\bsk-ant-[A-Za-z0-9_-]{8,}"),
    (SecretKind::OpenAi, r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}"),
    (
        SecretKind::GitHub,
        r"\b(?:gh[pos]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})",
    ),
    (SecretKind::Aws, r"\bAKIA[0-9A-Z]{16}\b"),
    (SecretKind::Slack, r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
    (SecretKind::Google, r"\bAIza[0-9A-Za-z_-]{35}"),
    (
        SecretKind::Bearer,
        r"\b(?i:bearer)\s+[A-Za-z0-9._~+/=-]{16,}",
    ),
    (
        SecretKind::Jwt,
        r"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}",
    ),
    (
        SecretKind::PrivateKey,
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
    ),
];

static COMPILED: LazyLock<Vec<(SecretKind, Regex)>> = LazyLock::new(|| {
    PATTERNS
        .iter()
        .filter_map(|(k, p)| Regex::new(p).ok().map(|r| (*k, r)))
        .collect()
});

static REGEXES: LazyLock<Vec<Regex>> =
    LazyLock::new(|| COMPILED.iter().map(|(_, r)| r.clone()).collect());

/// Compiled provider-key patterns, for the redactor.
pub fn key_patterns() -> &'static [Regex] {
    &REGEXES
}

fn entropy(s: &str) -> f64 {
    let mut counts = [0u32; 256];
    for b in s.bytes() {
        counts[b as usize] += 1;
    }
    let n = s.len() as f64;
    counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = f64::from(*c) / n;
            -p * p.log2()
        })
        .sum()
}

fn generic(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "_-+/=".contains(c)))
        .any(|w| {
            w.len() >= 32
                && w.bytes().any(|b| b.is_ascii_alphabetic())
                && w.bytes().any(|b| b.is_ascii_digit())
                && entropy(w) >= 4.0
        })
}

/// Return the kind of secret `text` appears to contain, if any.
pub fn looks_secret(text: &str) -> Option<SecretKind> {
    COMPILED
        .iter()
        .find(|(_, r)| r.is_match(text))
        .map(|(k, _)| *k)
        .or_else(|| generic(text).then_some(SecretKind::Generic))
}

#[cfg(test)]
mod tests {
    use super::*;
    use SecretKind::*;

    #[test]
    fn detects_each_kind() {
        let cases = [
            (concat!("sk-a", "nt-api03-abcdefghijkl"), Anthropic),
            (concat!("key sk-p", "roj-abcdefghijklmnopqrstuvwx"), OpenAi),
            (concat!("sk-a", "bcdefghijklmnopqrstuvwx"), OpenAi),
            (concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123"), GitHub),
            (concat!("gith", "ub_pat_abcdefghijklmnopqrstuv"), GitHub),
            (concat!("AKIA", "ABCDEFGHIJKLMNOP"), Aws),
            (concat!("xoxb", "-1234567890-abcdef"), Slack),
            // Split so secret scanners do not flag the fake key.
            (
                concat!("AIza", "SyA1234567890abcdefghijklmnopqrstuvw"),
                Google,
            ),
            ("Authorization: Bearer abcdefghijklmnop1234", Bearer),
            (
                concat!("eyJh", "bGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.c2lnbmF0dXJl"),
                Jwt,
            ),
            ("-----BEGIN RSA PRIVATE KEY-----", PrivateKey),
            ("aB3xK9mQ2vL7nP4wR8tY6uI1oE5zC0dF", Generic),
        ];
        for (t, k) in cases {
            assert_eq!(looks_secret(t), Some(k), "{t}");
        }
    }

    #[test]
    fn bearer_any_case() {
        for t in [
            "bearer abcdefghijklmnop1234",
            "BEARER abcdefghijklmnop1234",
            "Authorization: bEaReR abcdefghijklmnop1234",
        ] {
            assert_eq!(looks_secret(t), Some(Bearer), "{t}");
        }
    }

    #[test]
    fn key_patterns_start_with_word_boundary() {
        for (k, p) in PATTERNS {
            if !matches!(k, Jwt | PrivateKey) {
                assert!(p.starts_with(r"\b"), "{k:?}: {p}");
            }
        }
    }

    #[test]
    fn keys_after_separator_and_quoted_json() {
        assert_eq!(
            looks_secret(concat!(
                "key=sk-p",
                "roj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN"
            )),
            Some(OpenAi)
        );
        assert_eq!(
            looks_secret(r#"  "api_key": "aB3xK9mQ2vL7nP4wR8tY6uI1oE5zC0dF","#),
            Some(Generic)
        );
    }

    #[test]
    fn keys_detected_at_start_and_after_separators() {
        let keys = [
            (concat!("sk-a", "nt-api03-abcdefghijkl"), Anthropic),
            (concat!("sk-p", "roj-abcdefghijklmnopqrstuvwx"), OpenAi),
            (concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123"), GitHub),
            (concat!("xoxb", "-1234567890-abcdef"), Slack),
            // Split so secret scanners do not flag the fake key.
            (
                concat!("AIza", "SyA1234567890abcdefghijklmnopqrstuvw"),
                Google,
            ),
        ];
        for (k, kind) in keys {
            for prefix in ["", " ", "=", "\"", ": \""] {
                let t = format!("{prefix}{k}");
                assert_eq!(looks_secret(&t), Some(kind), "{t}");
            }
        }
    }

    #[test]
    fn no_false_positives() {
        for t in [
            "",
            "the quick brown fox jumps over the lazy dog, twice",
            r"C:\Users\someone\Documents\report-final-version-2.docx",
            "/home/user/projects/launcher/src/main.rs",
            "https://example.com/some/path?q=hello",
            "thisisalongwordwithoutanydigitsatallokay",
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            concat!("risk-a", "ssessment-template-final.docx"),
        ] {
            assert_eq!(looks_secret(t), None, "{t}");
        }
    }

    #[test]
    fn patterns_compile() {
        assert_eq!(key_patterns().len(), PATTERNS.len());
    }
}
