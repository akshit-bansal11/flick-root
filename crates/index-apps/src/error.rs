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
    /// The app cache could not be read.
    #[error("app cache unreadable")]
    Cache(#[source] std::io::Error),
}
