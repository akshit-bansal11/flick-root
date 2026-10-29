//! Provider trait, OpenAI-compatible and Anthropic clients, presets, local detection, fallback order, cancellation and the secret store per ADR-011.

pub mod anthropic;
pub mod auth;
pub mod detect;
pub mod error;
pub mod http;
pub mod hub;
pub mod openai_compat;
pub mod presets;
pub mod secret;
pub mod sse;
pub(crate) mod stream;
pub mod types;

use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio_util::sync::CancellationToken;

pub use auth::{KeySource, NoKeys};
pub use error::ProviderError;
pub use openai_compat::OpenAiCompat;
pub use types::*;

/// A streaming chat backend (ADR-006).
#[async_trait]
pub trait Provider: Send + Sync {
    /// The configuration this provider was built from.
    fn config(&self) -> &ProviderConfig;
    /// Start a streamed completion; cancelling the token drops the connection.
    fn stream(
        &self,
        req: Request,
        cancel: CancellationToken,
    ) -> BoxStream<'static, Result<Delta, ProviderError>>;
    /// Check reachability and list models (GET /models).
    async fn probe(&self) -> ProbeResult;
    /// Read the server's model list with each model's effort levels, at most
    /// [`MAX_FETCHED_MODELS`]. The default reports that listing is unsupported.
    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Err(ProviderError::Decode("model list not supported"))
    }
}
