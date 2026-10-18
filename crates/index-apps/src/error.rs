//! Errors for the apps index.

/// Errors from app enumeration, launch and caching.
#[derive(thiserror::Error, Debug)]
pub enum AppsError {
    /// Shell enumeration of AppsFolder failed.
    #[error("shell enumeration failed")]
    Enumerate(#[source] windows::core::Error),
    /// Launching an app failed.
    #[error("launch failed for {name}")]
    Launch {
        /// App display name.
        name: String,
        /// Underlying Windows error.
        #[source]
        source: windows::core::Error,
    },
    /// The user dismissed the elevation prompt.
    #[error("launch cancelled by the user")]
    Cancelled,
    /// Spawning a helper process (explorer) failed.
    #[error("could not start explorer")]
    Spawn(#[source] std::io::Error),
    /// Icon extraction or encoding failed.
    #[error("icon extraction failed: {0}")]
    Icon(String),
    /// The app cache could not be read or written.
    #[error("app cache unreadable")]
    Cache(#[source] std::io::Error),
}
