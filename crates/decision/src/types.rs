//! Shared decision-engine types. No trait impls or network code yet.

use serde::{Deserialize, Serialize};

/// A candidate file: name and parent folder only, never contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// File name.
    pub name: String,
    /// Parent folder path.
    pub parent: String,
}

/// A single Choice question sent to a decision engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceQuestion {
    /// The user's phrase.
    pub phrase: String,
    /// At most 8 candidates.
    pub candidates: Vec<Candidate>,
}

/// An engine's answer to a [`ChoiceQuestion`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChoiceAnswer {
    /// Index into the question's candidates.
    pub index: usize,
    /// Engine confidence, 0.0 to 1.0.
    pub p: f32,
    /// Id of the engine that answered ("laya", "kev", "llm", "jev").
    pub engine: &'static str,
}

/// Errors from a decision engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The engine could not be reached.
    #[error("engine unreachable: {0}")]
    Unreachable(String),
    /// The engine replied with something unusable.
    #[error("bad engine response: {0}")]
    BadResponse(String),
    /// The engine timed out.
    #[error("engine timed out")]
    Timeout,
}

/// Result of probing an engine or provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Engine id.
    pub id: String,
    /// Whether the endpoint answered.
    pub reachable: bool,
    /// Human-readable detail on failure.
    pub detail: Option<String>,
}
