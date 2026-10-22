//! Shared types, Mode, nucleo matcher wrapper, ranking hooks, SQLite store and the brand constants. Pure logic with no UI or Windows calls.

pub mod brand;
pub mod config;
pub mod detect;
pub mod error;
pub mod matcher;
pub mod ranking;
pub mod redact;
pub mod store;
pub mod types;

pub use config::Config;
pub use detect::{SecretKind, looks_secret};
pub use error::CoreError;
pub use types::{Action, ActionKind, IconRef, Input, Item, ItemId, ItemKind, Mode, Outcome};
