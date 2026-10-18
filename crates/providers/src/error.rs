//! Provider errors. Messages never carry secrets or user text.

/// Errors from provider clients.
#[derive(thiserror::Error, Debug)]
pub enum ProviderError {
    /// Server answered non-2xx (body discarded).
    #[error("server returned HTTP {code}")]
    Status {
        /// HTTP status code.
        code: u16,
    },
    /// Could not connect or the connection broke.
    #[error("network error: {0}")]
    Network(String),
    /// Timed out waiting for the named phase.
    #[error("timed out waiting for {phase}")]
    Timeout {
        /// Phase that timed out.
        phase: &'static str,
    },
    /// Cancelled by the caller.
    #[error("cancelled")]
    Cancelled,
    /// Response could not be decoded.
    #[error("bad response: {0}")]
    Decode(&'static str),
    /// A key is required but none is stored.
    #[error("no key stored for {host}")]
    NoKey {
        /// Target host.
        host: String,
    },
    /// Key is bound to a different host than the request target.
    #[error("key is bound to {bound}, request goes to {actual}")]
    HostMismatch {
        /// Host the key was entered for.
        bound: String,
        /// Host of the request.
        actual: String,
    },
    /// Refusing to send a key over plain HTTP to a non-loopback host.
    #[error("refusing to send a key over insecure http to {host}")]
    InsecureUrl {
        /// Target host.
        host: String,
    },
}

impl ProviderError {
    /// True for connect/timeout/429/5xx: try the next provider.
    pub fn is_fallback_worthy(&self) -> bool {
        match self {
            Self::Network(_) | Self::Timeout { .. } => true,
            Self::Status { code } => *code == 429 || *code >= 500,
            _ => false,
        }
    }
}
