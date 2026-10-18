//! Key lookup at request time. Keys are never stored in client structs.

use crate::{ProviderConfig, ProviderError};
use secrecy::SecretString;

/// Supplies the API key for a provider, on demand.
pub trait KeySource: Send + Sync {
    /// Fetch the key now; the caller drops it right after building the header.
    fn key_for(&self, cfg: &ProviderConfig) -> Result<Option<SecretString>, ProviderError>;
}

/// A source with no keys (local servers).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoKeys;

impl KeySource for NoKeys {
    fn key_for(&self, _cfg: &ProviderConfig) -> Result<Option<SecretString>, ProviderError> {
        Ok(None)
    }
}
