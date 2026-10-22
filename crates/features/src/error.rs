//! Feature error type.

/// Errors a feature or the registry can return.
#[derive(thiserror::Error, Debug)]
pub enum FeatureError {
    /// `start()` failed.
    #[error("feature {id} failed to start: {reason}")]
    Start {
        /// Feature id.
        id: &'static str,
        /// Human-readable cause.
        reason: String,
    },
    /// Options failed schema validation.
    #[error("invalid options: {0}")]
    Options(String),
    /// A confirm token expired or was reused.
    #[error("confirmation expired")]
    ConfirmExpired,
    /// Text looked like a secret and was blocked.
    #[error("blocked: text looks like a secret ({0})")]
    SecretBlocked(&'static str),
    /// No feature has this id.
    #[error("unknown feature {0}")]
    Unknown(String),
    /// Any other failure, with context.
    #[error("{0}")]
    Other(String),
}
