//! File index: scan, skip rules, filesystem watcher, snapshot persistence and query over indexed paths for Find mode.

pub mod error;
pub mod query;
pub mod scan;
pub mod skip;
pub mod snapshot;
pub mod store;
pub mod watch;

pub use error::FilesError;
pub use query::{FileHit, FindQuery, query};
pub use scan::{Root, default_roots, scan};
pub use store::{FileIndex, FileStore, StoreBuilder};
pub use watch::{Overlay, WatchHandle};

impl FileIndex {
    /// Start watching `roots`; the handle's overlay feeds [`FileIndex::find`].
    pub fn watch(roots: &[Root]) -> Result<WatchHandle, FilesError> {
        WatchHandle::start(roots)
    }

    /// Query the current store merged with `overlay`.
    pub fn find(&self, overlay: &Overlay, q: &FindQuery) -> Vec<FileHit> {
        query(&self.load(), overlay, q)
    }

    /// Fold `overlay` into a new store and publish it. Call when the overlay passes ~5,000 entries.
    pub fn compact(&self, overlay: &Overlay) {
        self.replace(watch::fold(&self.load(), overlay));
        overlay.clear();
    }
}
