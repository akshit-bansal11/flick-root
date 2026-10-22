//! Compact arena store for indexed paths.
//!
//! One `String` holds every name; each entry is 16 bytes (name offset/len, parent, flags,
//! root id, modified minutes). 300k entries is roughly 5 MB of entries plus names, well
//! under the 60 MB target. Names are kept as UTF-8; the query card converts to UTF-32
//! lazily per candidate (nucleo borrows ASCII directly), avoiding 4x memory.
//! Root entries store the full root path as their name; children store the file name.

use crate::error::FilesError;
use crate::scan::Root;
use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Parent index marking a root entry.
pub const NO_PARENT: u32 = u32::MAX;

/// Most entries a store holds, so the store stays within 64 MB (67,108,864 bytes).
///
/// Bytes per entry: the [`Entry`] struct is 4 (name_off), 2 (name_len), 1 (is_dir), 1 (root),
/// 4 (parent) and 4 (modified_min), 16 bytes in all. The shared name arena adds the file
/// name, about 24 bytes on average (UTF-8, no terminator). That is 40 bytes per entry, and
/// 67,108,864 / 40 = 1,677,721 entries; 1,600,000 leaves about 4% headroom
/// (1,600,000 x 40 = 64,000,000 bytes).
pub const MAX_FILES: usize = 1_600_000;

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

    /// Entries pushed so far.
    pub fn len(&self) -> usize {
        self.store.entries.len()
    }

    /// True when nothing was pushed.
    pub fn is_empty(&self) -> bool {
        self.store.entries.is_empty()
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

    /// A new store holding this one plus the entries of `scanned` filed under `root.id`,
    /// with no I/O. `scanned` is typically `scan(&[root])`. A root with the same path is
    /// replaced. Root ids are reassigned in table order (existing roots first, `root` last).
    pub fn with_root(&self, root: &Root, scanned: &FileStore) -> Result<FileStore, FilesError> {
        let kept = self
            .roots
            .iter()
            .filter(|r| r.path != root.path)
            .cloned()
            .collect();
        let (mut roots, ids) = renumber(&self.roots, kept);
        let new_id =
            u8::try_from(roots.len()).map_err(|_| FilesError::TooManyRoots(root.path.clone()))?;
        let mut b = StoreBuilder::new();
        copy_into(&mut b, self, &ids, |_| true);
        let mut scanned_ids = [None; 256];
        scanned_ids[usize::from(root.id)] = Some(new_id);
        copy_into(&mut b, scanned, &scanned_ids, |_| true);
        roots.push(Root {
            id: new_id,
            ..root.clone()
        });
        Ok(b.finish(roots))
    }

    /// A new store without the root whose path is `path` (and its entries); other root ids
    /// are reassigned. No I/O.
    pub fn without_root(&self, path: &Path) -> FileStore {
        let kept = self
            .roots
            .iter()
            .filter(|r| r.path != path)
            .cloned()
            .collect();
        let (roots, ids) = renumber(&self.roots, kept);
        let mut b = StoreBuilder::new();
        copy_into(&mut b, self, &ids, |_| true);
        b.finish(roots)
    }

    /// A new store without `prefix` and everything beneath it: a subtree inside a root, or
    /// whole roots that sit under it. No I/O.
    pub fn without_prefix(&self, prefix: &Path) -> FileStore {
        let kept = self
            .roots
            .iter()
            .filter(|r| !r.path.starts_with(prefix))
            .cloned()
            .collect();
        let (roots, ids) = renumber(&self.roots, kept);
        let leaf = prefix.file_name().and_then(|n| n.to_str());
        let mut b = StoreBuilder::new();
        // Children of a dropped entry are dropped by copy_into, so only the top match is tested.
        copy_into(&mut b, self, &ids, |i| {
            if self.parent(i).is_none() {
                return !Path::new(self.name(i)).starts_with(prefix);
            }
            !(leaf == Some(self.name(i)) && self.path_of(i) == prefix)
        });
        b.finish(roots)
    }
}

/// Assign ids 0.. to `kept` in order; returns the renumbered roots and a map from each old
/// id in `old` to its new id (`None` when that root was dropped).
fn renumber(old: &[Root], mut kept: Vec<Root>) -> (Vec<Root>, [Option<u8>; 256]) {
    let mut ids = [None; 256];
    for (n, r) in kept.iter_mut().enumerate() {
        let Ok(new) = u8::try_from(n) else { break };
        if old.iter().any(|o| o.id == r.id && o.path == r.path) {
            ids[usize::from(r.id)] = Some(new);
        }
        r.id = new;
    }
    kept.truncate(256);
    (kept, ids)
}

/// Append the entries of `src` that `keep` accepts and whose root id maps through `ids`,
/// remapping parents. An entry whose parent was not copied is dropped with it. Relies on
/// parents preceding children, which scan and fold guarantee.
fn copy_into(
    b: &mut StoreBuilder,
    src: &FileStore,
    ids: &[Option<u8>; 256],
    keep: impl Fn(usize) -> bool,
) {
    let mut map = vec![NO_PARENT; src.len()];
    for (i, e) in src.entries.iter().enumerate() {
        let Some(root) = ids[usize::from(e.root)] else {
            continue;
        };
        let parent = if e.parent == NO_PARENT {
            NO_PARENT
        } else {
            match map.get(e.parent as usize) {
                Some(&p) if p != NO_PARENT => p,
                _ => continue,
            }
        };
        if !keep(i) {
            continue;
        }
        if let (Some(n), Some(slot)) = (
            b.push(src.name(i), parent, e.is_dir, e.modified_min, root),
            map.get_mut(i),
        ) {
            *slot = n;
        }
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
