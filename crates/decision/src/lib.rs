//! Decision engines: the NL parser (stage 1, offline), shared engine types, and later the /v1/systemone client for Laya, Kev and Jev and the LLM picker with strict JSON parsing. Receives file names and parent folders only.

mod parser;
mod types;

pub use parser::{DateRange, KnownRoot, Parsed, is_executable, is_natural, parse};
pub use types::{Candidate, ChoiceAnswer, ChoiceQuestion, EngineError, ProbeResult};
