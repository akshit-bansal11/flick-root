//! Error type for the file index.

/// Errors from scanning, snapshotting or querying the file index.
#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    /// The scan was cancelled through the cancel flag.
    #[error("scan cancelled")]
    Cancelled,
    /// A root could not be added: root ids are a `u8`, so at most 256 roots fit.
    #[error("cannot add {0}: at most 256 indexed roots")]
    TooManyRoots(std::path::PathBuf),
    /// An I/O operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being attempted.
        context: String,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl FilesError {
    /// Wrap an I/O error with context.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
