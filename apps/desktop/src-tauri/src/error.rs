//! Error type returned by every Tauri command (`{ code, message }`).
use std::fmt;

/// Serializable command error. `message` must never contain secrets, key
/// material or captured user text; it is shown in the UI and logged.
#[derive(Debug, serde::Serialize)]
pub struct CommandError {
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable, secret-free description.
    pub message: String,
}

impl CommandError {
    /// Builds an error from a stable code and a secret-free message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<anyhow::Error> for CommandError {
    fn from(err: anyhow::Error) -> Self {
        Self::new("internal", format!("{err:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anyhow_maps_to_internal() {
        let e: CommandError = anyhow::anyhow!("boom").into();
        assert_eq!(e.code, "internal");
        assert_eq!(e.message, "boom");
    }
}
