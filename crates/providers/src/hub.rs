//! Provider hub: ordering, fallback and cancellation.

use crate::presets::{build_provider, preset};
use crate::{Delta, KeySource, Provider, ProviderError, Request};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use futures::stream::BoxStream;
use launcher_core::config::Providers;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Events of one ask; mirrors BSD section 5 `AskEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubEvent {
    /// First provider selected.
    Started {
        /// Provider id.
        provider: String,
        /// Model name.
        model: String,
    },
    /// Moved on to the next provider before any text arrived.
    Fallback {
        /// Failed provider id.
        from: String,
        /// Next provider id.
        to: String,
        /// Short reason, no secrets or bodies.
        reason: String,
    },
    /// The model is reasoning.
    Thinking,
    /// Answer text.
    Delta {
        /// Text chunk.
        text: String,
    },
    /// Finished.
    Done {
        /// Provider that answered.
        provider: String,
    },
    /// Failed for good.
    Error {
        /// Machine code.
        code: String,
        /// Human message, no secrets or bodies.
        message: String,
    },
}

/// Enabled providers in fallback order.
pub struct ProviderHub {
    providers: Vec<Arc<dyn Provider>>,
    first_token: Duration,
}

/// Longest wait for the request to be answered with a first delta.
pub const FIRST_TOKEN_TIMEOUT: Duration = Duration::from_secs(10);

fn err(code: &str, message: String) -> HubEvent {
    HubEvent::Error {
        code: code.to_owned(),
        message,
    }
}

impl ProviderHub {
    /// Hub over already-built providers, tried in the given order.
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Self {
        Self {
            providers,
            first_token: FIRST_TOKEN_TIMEOUT,
        }
    }

    /// Override the first-token timeout.
    pub fn with_first_token_timeout(mut self, d: Duration) -> Self {
        self.first_token = d;
        self
    }

    /// Build enabled providers in `order`; entries that fail to build are skipped.
    pub fn from_config(cfg: &Providers, keys: &Arc<dyn KeySource>) -> Self {
        let custom = preset("custom");
        let providers = cfg
            .order
            .iter()
            .filter_map(|id| {
                let entry = cfg.entries.get(id).filter(|e| e.enabled)?;
                let pr = preset(id).or(custom)?;
                build_provider(id, entry, pr, keys.clone()).ok()
            })
            .map(Arc::from)
            .collect();
        Self::new(providers)
    }

    /// True when no provider is enabled.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Ask with fallback. Cancelling stops everything and emits nothing further.
    pub fn ask(&self, req: Request, cancel: CancellationToken) -> BoxStream<'static, HubEvent> {
        let (tx, rx) = unbounded();
        let providers = self.providers.clone();
        let first_token = self.first_token;
        tokio::spawn(async move {
            tokio::select! {
                () = cancel.cancelled() => {}
                () = run(providers, req, cancel.clone(), first_token, &tx) => {}
            }
        });
        rx.boxed()
    }
}

async fn run(
    providers: Vec<Arc<dyn Provider>>,
    req: Request,
    cancel: CancellationToken,
    first_token: Duration,
    tx: &UnboundedSender<HubEvent>,
) {
    let send = |e: HubEvent| {
        let _ = tx.unbounded_send(e);
    };
    let Some(first) = providers.first() else {
        send(err("no_provider", "no provider is enabled".into()));
        return;
    };
    send(HubEvent::Started {
        provider: first.config().id.clone(),
        model: first.config().model.clone(),
    });
    for (i, p) in providers.iter().enumerate() {
        let id = p.config().id.clone();
        let mut stream = p.stream(req.clone(), cancel.clone());
        let mut started = false;
        let mut first = true;
        let deadline = tokio::time::Instant::now() + first_token;
        let failure: ProviderError = loop {
            let item = if first {
                match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(item) => item,
                    Err(_) => {
                        break ProviderError::Timeout {
                            phase: "first token",
                        };
                    }
                }
            } else {
                stream.next().await
            };
            first = false;
            match item {
                Some(Ok(Delta::Text(text))) => {
                    started = true;
                    send(HubEvent::Delta { text });
                }
                Some(Ok(Delta::Thinking)) => send(HubEvent::Thinking),
                Some(Ok(Delta::Done(_))) | None => {
                    send(HubEvent::Done { provider: id });
                    return;
                }
                Some(Err(ProviderError::Cancelled)) => return,
                Some(Err(e)) => break e,
            }
        };
        if started {
            send(err("stream_failed", failure.to_string()));
            return;
        }
        if !failure.is_fallback_worthy() {
            send(err("provider_error", failure.to_string()));
            return;
        }
        match providers.get(i + 1) {
            Some(next) => send(HubEvent::Fallback {
                from: id,
                to: next.config().id.clone(),
                reason: failure.to_string(),
            }),
            None => send(err("all_failed", "every provider failed".into())),
        }
    }
}
