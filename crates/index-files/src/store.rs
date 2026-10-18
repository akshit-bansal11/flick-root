//! Compact arena store for indexed paths.
//!
//! One `String` holds every name; each entry is 16 bytes (name offset/len, parent, flags,
//! root id, modified minutes). 300k entries is roughly 5 MB of entries plus names, well
//! under the 60 MB target. Names are kept as UTF-8; the query card converts to UTF-32
//! lazily per candidate (nucleo borrows ASCII directly), avoiding 4x memory.
//! Root entries store the full root path as their name; children store the file name.

use crate::scan::Root;
use arc_swap::ArcSwap;
use std::path::PathBuf;
use std::sync::Arc;

/// Parent index marking a root entry.
pub const NO_PARENT: u32 = u32::MAX;

/// One indexed file or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub(crate) name_off: u32,
    pub(crate) name_len: u16,
    pub(crate) is_dir: bool,
    pub(crate) root: u8,
    pub(crate) parent: u32,
    pub(crate) modified_min: u32,
}

/// Immutable set of indexed entries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileStore {
    pub(crate) roots: Vec<Root>,
    pub(crate) names: String,
    pub(crate) entries: Vec<Entry>,
}

/// Incrementally builds a [`FileStore`].
#[derive(Debug, Default)]
pub struct StoreBuilder {
    store: FileStore,
}

impl StoreBuilder {
    /// Start an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an entry and return its index. Over-long names or a full arena are dropped (`None`).
    pub fn push(
        &mut self,
        name: &str,
        parent: u32,
        is_dir: bool,
        modified_min: u32,
        root: u8,
    ) -> Option<u32> {
        let name_len = u16::try_from(name.len()).ok()?;
        let name_off = u32::try_from(self.store.names.len()).ok()?;
        let idx = u32::try_from(self.store.entries.len())
            .ok()
            .filter(|i| *i != NO_PARENT)?;
        self.store.names.push_str(name);
        self.store.entries.push(Entry {
            name_off,
            name_len,
            is_dir,
            root,
            parent,
            modified_min,
        });
        Some(idx)
    }

    /// Finish with the root table.
    pub fn finish(mut self, roots: Vec<Root>) -> FileStore {
        self.store.roots = roots;
        self.store.names.shrink_to_fit();
        self.store.entries.shrink_to_fit();
        self.store
    }
}

impl FileStore {
    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Scan roots.
    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    /// Name of entry `idx` (empty if out of range).
    pub fn name(&self, idx: usize) -> &str {
        self.entries.get(idx).map_or("", |e| {
            let s = e.name_off as usize;
            self.names.get(s..s + e.name_len as usize).unwrap_or("")
        })
    }

    /// Whether entry `idx` is a directory.
    pub fn is_dir(&self, idx: usize) -> bool {
        self.entries.get(idx).is_some_and(|e| e.is_dir)
    }

    /// Root id of entry `idx`.
    pub fn root_id(&self, idx: usize) -> Option<u8> {
        self.entries.get(idx).map(|e| e.root)
    }

    /// Modified time in minutes since the Unix epoch.
    pub fn modified_min(&self, idx: usize) -> Option<u32> {
        self.entries.get(idx).map(|e| e.modified_min)
    }

    /// Parent index, or `None` for roots.
    pub fn parent(&self, idx: usize) -> Option<usize> {
        self.entries
            .get(idx)
            .map(|e| e.parent)
            .filter(|p| *p != NO_PARENT)
            .map(|p| p as usize)
    }

    /// Full path reconstructed by walking parents.
    pub fn path_of(&self, idx: usize) -> PathBuf {
        let mut chain = Vec::new();
        let mut cur = Some(idx);
        while let Some(i) = cur.filter(|i| *i < self.entries.len()) {
            chain.push(i);
            cur = self.parent(i);
            if chain.len() > 4096 {
                break; // cycle guard
            }
        }
        let mut p = PathBuf::new();
        for i in chain.into_iter().rev() {
            p.push(self.name(i));
        }
        p
    }
}

/// Lock-free swappable handle to the current store.
#[derive(Debug, Default)]
pub struct FileIndex {
    inner: ArcSwap<FileStore>,
}

impl FileIndex {
    /// Empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically publish a new store.
    pub fn replace(&self, store: FileStore) {
        self.inner.store(Arc::new(store));
    }

    /// Current store snapshot.
    pub fn load(&self) -> Arc<FileStore> {
        self.inner.load_full()
    }

    /// Entries in the current store.
    pub fn len(&self) -> usize {
        self.inner.load().len()
    }

    /// True when the current store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Full path of entry `idx` in the current store.
    pub fn path_of(&self, idx: usize) -> PathBuf {
        self.inner.load().path_of(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_swap() {
        let mut b = StoreBuilder::new();
        let r = b.push("C:\\Users\\x", NO_PARENT, true, 0, 0).unwrap();
        let d = b.push("docs", r, true, 1, 0).unwrap();
        let f = b.push("a.txt", d, false, 2, 0).unwrap();
        let s = b.finish(vec![]);
        assert_eq!(
            s.path_of(f as usize),
            PathBuf::from("C:\\Users\\x\\docs\\a.txt")
        );
        assert!(!s.is_dir(f as usize));
        let ix = FileIndex::new();
        assert!(ix.is_empty());
        ix.replace(s);
        assert_eq!(ix.len(), 3);
        assert_eq!(ix.path_of(1), PathBuf::from("C:\\Users\\x\\docs"));
    }
}
