//! File index: scan, skip rules, filesystem watcher, snapshot persistence and query over indexed paths for Find mode.

use std::sync::Arc;

pub mod drives;
pub mod error;
pub mod programs;
pub mod query;
pub mod scan;
pub mod skip;
pub mod skiplist;
pub mod snapshot;
pub mod store;
pub mod watch;

pub use error::FilesError;
pub use programs::{PROGRAM_EXTS, ProgramCache, ProgramFile, ScannedRoot, scan_root};
pub use query::{FileHit, FindQuery, query};
pub use scan::{Parallelism, Root, default_roots, scan, scan_capped};
pub use skiplist::SkipList;
pub use store::{FileIndex, FileStore, MAX_FILES, StoreBuilder};
pub use watch::{Overlay, ProgramChange, ProgramHook, WatchHandle};

impl FileIndex {
    /// Start watching `roots`; the handle's overlay feeds [`FileIndex::find`]. Changes under a
    /// directory `skip` excludes are ignored.
    pub fn watch(
        roots: &[Root],
        skip: &SkipList,
        on_program: Option<ProgramHook>,
    ) -> Result<WatchHandle, FilesError> {
        WatchHandle::start(roots, skip, None, on_program)
    }

    /// Like [`FileIndex::watch`], but compacts into this index in the background when the
    /// overlay grows past [`watch::COMPACT_AT`].
    pub fn watch_auto(
        self: &Arc<Self>,
        roots: &[Root],
        skip: &SkipList,
        on_program: Option<ProgramHook>,
    ) -> Result<WatchHandle, FilesError> {
        WatchHandle::start(roots, skip, Some(Arc::clone(self)), on_program)
    }

    /// Query the current store merged with `overlay`.
    pub fn find(&self, overlay: &Overlay, q: &FindQuery) -> Vec<FileHit> {
        query(&self.load(), overlay, q)
    }

    /// Fold `overlay` into a new store and publish it. Holds the overlay's write lock so no
    /// event is lost between fold and clear (ponytail: queries stall during the fold).
    pub fn compact(&self, overlay: &Overlay) {
        let mut g = overlay.write();
        self.replace(watch::fold(&self.load(), &g));
        g.reset();
    }
}
