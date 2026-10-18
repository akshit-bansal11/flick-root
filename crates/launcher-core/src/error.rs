//! Error type for launcher-core (BSD section 10). Messages never carry secrets or user text.

use std::path::PathBuf;

/// Errors returned by launcher-core.
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    /// config.toml could not be parsed.
    #[error("config parse failed at {path}")]
    ConfigParse {
        /// File that failed to parse.
        path: PathBuf,
        /// Underlying TOML error.
        #[source]
        source: toml::de::Error,
    },
    /// config.toml could not be read or written.
    #[error("config io failed at {path}")]
    ConfigIo {
        /// File involved.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// The config could not be serialized.
    #[error("config serialize failed")]
    ConfigSerialize(#[from] toml::ser::Error),
    /// A required environment variable (APPDATA, LOCALAPPDATA) is missing.
    #[error("environment variable {0} is not set")]
    MissingEnv(&'static str),
    /// SQLite error.
    #[error("store error: {0}")]
    Store(#[from] rusqlite::Error),
    /// The store was corrupt and has been reset.
    #[error("store was reset")]
    StoreReset,
}
