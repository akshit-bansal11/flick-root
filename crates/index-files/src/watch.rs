//! Filesystem watcher keeping the file index current between scans.
//!
//! Changes land in an [`Overlay`] (added entries plus removed paths) that queries merge with
//! the immutable store; [`FileIndex::compact`] folds it back in.

use crate::error::FilesError;
use crate::scan::Root;
use crate::skip::should_skip;
use crate::store::{FileIndex, FileStore, NO_PARENT, StoreBuilder};
use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::UNIX_EPOCH;

/// Overlay entries beyond this trigger a background compaction.
pub const COMPACT_AT: usize = 5_000;
/// Most entries recorded when a whole directory appears at once (rename or move into a root).
const WALK_CAP: usize = 50_000;

/// Entry created after the last scan.
#[derive(Debug, Clone)]
pub(crate) struct Added {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) root: u8,
    pub(crate) modified_min: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Inner {
    pub(crate) added: Vec<Added>,
    /// Paths deleted since the scan; hides matching store entries and their descendants.
    pub(crate) removed: HashSet<PathBuf>,
    /// Bumped whenever `removed` changes; keys the resolved-bitset cache.
    version: u64,
}

impl Inner {
    pub(crate) fn reset(&mut self) {
        self.added.clear();
        self.removed.clear();
        self.version += 1;
    }
}

/// Removed paths resolved to store indices (entry plus descendants) for one store and version.
#[derive(Debug)]
struct Gone {
    version: u64,
    store_key: (usize, usize),
    bits: Arc<Vec<u64>>,
}

/// One bit per store entry; true means the entry is hidden by the overlay.
pub(crate) fn bit(bits: &[u64], i: usize) -> bool {
    bits.get(i / 64).is_some_and(|w| (w >> (i % 64)) & 1 == 1)
}

/// Live changes layered over a [`FileStore`].
#[derive(Debug, Default)]
pub struct Overlay {
    inner: RwLock<Inner>,
    gone: Mutex<Option<Gone>>,
}

impl Overlay {
    /// Empty overlay.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Number of added entries plus removed paths.
    pub fn len(&self) -> usize {
        let g = self.read();
        g.added.len() + g.removed.len()
    }

    /// True when nothing is pending.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop all pending changes.
    pub fn clear(&self) {
        self.write().reset();
    }

    /// Bitset of store entries hidden by removals (each removed path and its descendants).
    /// Rebuilt with one pass over the store only after the overlay or store changes, so a
    /// query checks one bit per candidate and allocates nothing per candidate.
    pub(crate) fn gone_bits(&self, store: &FileStore, g: &Inner) -> Arc<Vec<u64>> {
        let key = (store.entries.as_ptr() as usize, store.len());
        let mut cache = self.gone.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(c) = cache.as_ref()
            && c.version == g.version
            && c.store_key == key
        {
            return Arc::clone(&c.bits);
        }
        let names: HashSet<&str> = g
            .removed
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .collect();
        let mut bits = vec![0u64; store.len().div_ceil(64)];
        for i in 0..store.len() {
            // Parents precede children in the store, so the parent's bit is already final.
            let hidden = store.parent(i).is_some_and(|p| bit(&bits, p))
                || (names.contains(store.name(i)) && g.removed.contains(&store.path_of(i)));
            if hidden && let Some(w) = bits.get_mut(i / 64) {
                *w |= 1 << (i % 64);
            }
        }
        let bits = Arc::new(bits);
        *cache = Some(Gone {
            version: g.version,
            store_key: key,
            bits: Arc::clone(&bits),
        });
        bits
    }

    /// Record that `path` was created or changed (exists on disk) or deleted (does not).
    /// A directory that appears is walked so its children are recorded too.
    pub fn apply_path(&self, roots: &[Root], path: &Path) {
        let md = std::fs::metadata(path).ok();
        let new = md.as_ref().map(|m| collect_added(roots, path, m));
        let track_removal = md.is_none() && removal_relevant(roots, path);
        let mut g = self.write();
        g.added.retain(|a| !a.path.starts_with(path));
        if track_removal {
            g.removed.insert(path.to_path_buf());
            g.version += 1;
        }
        g.added.extend(new.into_iter().flatten());
    }
}

/// A deletion only matters when the path sits under a root and no ancestor is a skipped name.
fn removal_relevant(roots: &[Root], path: &Path) -> bool {
    roots
        .iter()
        .find_map(|r| path.strip_prefix(&r.path).ok())
        .is_some_and(|rel| {
            !rel.components()
                .any(|c| should_skip(c.as_os_str(), 0, true))
        })
}

fn added_of(root: u8, path: PathBuf, name: &OsStr, md: &std::fs::Metadata) -> Added {
    let modified_min = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u32::try_from(d.as_secs() / 60).unwrap_or(u32::MAX));
    Added {
        name: name.to_string_lossy().into_owned(),
        is_dir: md.is_dir(),
        root,
        modified_min,
        path,
    }
}

/// The entry for `path` plus, for a directory, everything indexable beneath it.
fn collect_added(roots: &[Root], path: &Path, md: &std::fs::Metadata) -> Vec<Added> {
    let Some(first) = make_added(roots, path, md) else {
        return Vec::new();
    };
    let root = first.root;
    let mut out = vec![first];
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(cmd) = e.metadata() else { continue };
            let name = e.file_name();
            if should_skip(&name, cmd.file_attributes(), cmd.is_dir()) {
                continue;
            }
            if cmd.is_dir() {
                stack.push(e.path());
            }
            out.push(added_of(root, e.path(), &name, &cmd));
            if out.len() >= WALK_CAP {
                return out; // ponytail: huge moves are truncated; the next scan completes them
            }
        }
    }
    out
}

fn make_added(roots: &[Root], path: &Path, md: &std::fs::Metadata) -> Option<Added> {
    let root = roots.iter().find(|r| path.starts_with(&r.path))?;
    let rel = path.strip_prefix(&root.path).ok()?;
    let name = rel.file_name()?;
    if should_skip(name, md.file_attributes(), md.is_dir()) {
        return None;
    }
    // Every ancestor below the root must itself be indexable (name and hidden/system attributes).
    let mut cur = root.path.clone();
    for c in rel.parent().into_iter().flat_map(Path::components) {
        cur.push(c);
        let attrs = std::fs::metadata(&cur).map_or(0, |m| m.file_attributes());
        if should_skip(c.as_os_str(), attrs, true) {
            return None;
        }
    }
    Some(added_of(root.id, path.to_path_buf(), name, md))
}

/// Owns the OS watchers. Dropping it stops watching.
pub struct WatchHandle {
    _watcher: RecommendedWatcher,
    overlay: Arc<Overlay>,
}

impl WatchHandle {
    /// Start watching every root recursively. With `index`, the overlay is compacted into it
    /// on a background thread once it passes [`COMPACT_AT`] entries.
    pub fn start(roots: &[Root], index: Option<Arc<FileIndex>>) -> Result<Self, FilesError> {
        let overlay = Arc::new(Overlay::new());
        let (ov, rs) = (Arc::clone(&overlay), roots.to_vec());
        let busy = Arc::new(AtomicBool::new(false));
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            // Modify(data) is ignored: mtime refreshes on the next scan.
            if matches!(
                ev.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(_))
            ) {
                for p in &ev.paths {
                    ov.apply_path(&rs, p);
                }
                if let Some(ix) = &index
                    && ov.len() > COMPACT_AT
                    && !busy.swap(true, Ordering::AcqRel)
                {
                    let (ix, ov, flag) = (Arc::clone(ix), Arc::clone(&ov), Arc::clone(&busy));
                    let spawned = std::thread::Builder::new()
                        .name("files-compact".into())
                        .spawn(move || {
                            ix.compact(&ov);
                            flag.store(false, Ordering::Release);
                        });
                    if spawned.is_err() {
                        busy.store(false, Ordering::Release);
                    }
                }
            }
        })
        .map_err(|e| FilesError::io("create watcher", std::io::Error::other(e)))?;
        for r in roots {
            watcher
                .watch(&r.path, RecursiveMode::Recursive)
                .map_err(|e| {
                    FilesError::io(
                        format!("watch {}", r.path.display()),
                        std::io::Error::other(e),
                    )
                })?;
        }
        Ok(Self {
            _watcher: watcher,
            overlay,
        })
    }

    /// The overlay this watcher feeds.
    pub fn overlay(&self) -> Arc<Overlay> {
        Arc::clone(&self.overlay)
    }
}

/// Merge the overlay state `g` into `store`, producing a fresh store.
pub(crate) fn fold(store: &FileStore, g: &Inner) -> FileStore {
    let mut b = StoreBuilder::new();
    let mut map: Vec<Option<u32>> = vec![None; store.len()];
    let mut dirs: HashMap<PathBuf, u32> = HashMap::new();
    for i in 0..store.len() {
        let parent = match store.parent(i) {
            Some(p) => match map.get(p).copied().flatten() {
                Some(np) => np,
                None => continue,
            },
            None => NO_PARENT,
        };
        let path = store.path_of(i);
        if path.ancestors().any(|a| g.removed.contains(a)) {
            continue;
        }
        let new = b.push(
            store.name(i),
            parent,
            store.is_dir(i),
            store.modified_min(i).unwrap_or(0),
            store.root_id(i).unwrap_or(0),
        );
        map[i] = new;
        if let (Some(n), true) = (new, store.is_dir(i)) {
            dirs.insert(path, n);
        }
    }
    let mut added: Vec<&Added> = g.added.iter().collect();
    added.sort_by_key(|a| a.path.components().count()); // parents before children
    for a in added {
        let Some(&parent) = a.path.parent().and_then(|p| dirs.get(p)) else {
            continue; // parent unknown; next scan picks it up
        };
        if let Some(n) = b.push(&a.name, parent, a.is_dir, a.modified_min, a.root)
            && a.is_dir
        {
            dirs.insert(a.path.clone(), n);
        }
    }
    b.finish(store.roots().to_vec())
}
