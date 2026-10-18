//! Log and panic redaction: provider-key patterns plus registered literal secrets.

use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard};

use regex::{Regex, RegexSet};
use zeroize::Zeroizing;

use crate::detect::key_patterns;

const MASK: &str = "[REDACTED]";
/// Literal values shorter than this are ignored (too many false positives).
pub const MIN_SECRET_LEN: usize = 8;

/// Secret bytes zeroed on drop; Debug never prints them.
struct SecretBytes(Zeroizing<Vec<u8>>);

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(MASK)
    }
}

struct Inner {
    literals: RwLock<Vec<SecretBytes>>,
    /// `None` when the set failed to build: always take the slow path.
    set: Option<RegexSet>,
}

/// Masks key-shaped text and registered secrets. Cheap to clone.
#[derive(Clone)]
pub struct Redactor(Arc<Inner>);

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Redactor([REDACTED])")
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

impl Redactor {
    /// A redactor with the built-in key patterns and no literals.
    pub fn new() -> Self {
        let set = RegexSet::new(redaction_patterns().iter().map(Regex::as_str)).ok();
        Self(Arc::new(Inner {
            literals: RwLock::new(Vec::new()),
            set,
        }))
    }

    fn lits(&self) -> RwLockReadGuard<'_, Vec<SecretBytes>> {
        self.0.literals.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Registers a literal secret. Values under 8 bytes are ignored.
    pub fn register(&self, value: &[u8]) {
        if value.len() < MIN_SECRET_LEN {
            return;
        }
        self.0
            .literals
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(SecretBytes(Zeroizing::new(value.to_vec())));
    }

    /// Forgets (and zeroes) every registered literal.
    pub fn unregister_all(&self) {
        self.0
            .literals
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Replaces key patterns and registered secrets with `[REDACTED]`.
    /// Matches are found on the original text, merged, and masked once.
    /// Returns `Borrowed` when nothing matched.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let lits = self.lits();
        let bytes = text.as_bytes();
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for l in lits.iter() {
            let mut i = 0;
            while let Some(p) = find(bytes, &l.0, i) {
                ranges.push((p, p + l.0.len()));
                i = p + l.0.len();
            }
        }
        if self.0.set.as_ref().is_none_or(|s| s.is_match(text)) {
            for re in redaction_patterns() {
                ranges.extend(re.find_iter(text).map(|m| (m.start(), m.end())));
            }
        }
        if ranges.is_empty() {
            return Cow::Borrowed(text);
        }
        // Widen to char boundaries (a literal may be arbitrary bytes), then merge.
        for r in &mut ranges {
            while !text.is_char_boundary(r.0) {
                r.0 -= 1;
            }
            while !text.is_char_boundary(r.1) {
                r.1 += 1;
            }
        }
        ranges.sort_unstable();
        let mut out = String::with_capacity(text.len());
        let mut pos = 0;
        let mut cur = ranges[0];
        for &(s, e) in &ranges[1..] {
            if s <= cur.1 {
                cur.1 = cur.1.max(e);
            } else {
                out.push_str(&text[pos..cur.0]);
                out.push_str(MASK);
                pos = cur.1;
                cur = (s, e);
            }
        }
        out.push_str(&text[pos..cur.0]);
        out.push_str(MASK);
        out.push_str(&text[cur.1..]);
        Cow::Owned(out)
    }
}

/// Redaction-specific PEM pattern: header through END line, or to end of text.
const PEM_BLOCK: &str =
    r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)";

static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let mut v = key_patterns().to_vec();
    v.extend(Regex::new(PEM_BLOCK).ok());
    v
});

fn redaction_patterns() -> &'static [Regex] {
    &PATTERNS
}

static GLOBAL: LazyLock<Redactor> = LazyLock::new(Redactor::new);

/// Process-wide redactor for the logging layer and panic hook.
pub fn global() -> &'static Redactor {
    &GLOBAL
}
