//! Log and panic redaction: provider-key patterns plus registered literal secrets.

use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard};

use regex::RegexSet;
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
        let set = RegexSet::new(key_patterns().iter().map(|r| r.as_str())).ok();
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
    /// Returns `Borrowed` when nothing matched.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let lits = self.lits();
        let bytes = text.as_bytes();
        let has_lit = lits.iter().any(|l| find(bytes, &l.0, 0).is_some());
        if !has_lit && self.0.set.as_ref().is_some_and(|s| !s.is_match(text)) {
            return Cow::Borrowed(text);
        }
        let mut out = text.to_owned();
        for re in key_patterns() {
            if let Cow::Owned(s) = re.replace_all(&out, MASK) {
                out = s;
            }
        }
        for l in lits.iter() {
            let src = out.into_bytes();
            let mut buf = Vec::with_capacity(src.len());
            let mut i = 0;
            while let Some(p) = find(&src, &l.0, i) {
                buf.extend_from_slice(&src[i..p]);
                buf.extend_from_slice(MASK.as_bytes());
                i = p + l.0.len();
            }
            buf.extend_from_slice(&src[i..]);
            out = String::from_utf8_lossy(&buf).into_owned();
        }
        Cow::Owned(out)
    }
}

static GLOBAL: LazyLock<Redactor> = LazyLock::new(Redactor::new);

/// Process-wide redactor for the logging layer and panic hook.
pub fn global() -> &'static Redactor {
    &GLOBAL
}
