//! Filesystem watcher keeping the file index current between scans.
//!
//! Changes land in an [`Overlay`] (added entries plus removed paths) that queries merge with
//! the immutable store; [`FileIndex::compact`](crate::FileIndex::compact) folds it back in.

use crate::error::FilesError;
use crate::scan::Root;
use crate::skip::should_skip;
use crate::store::{FileStore, NO_PARENT, StoreBuilder};
use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard};
use std::time::UNIX_EPOCH;

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
}

/// Live changes layered over a [`FileStore`].
#[derive(Debug, Default)]
pub struct Overlay {
    inner: RwLock<Inner>,
}

impl Overlay {
    /// Empty overlay.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
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
        let mut g = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        g.added.clear();
        g.removed.clear();
    }

    /// Record that `path` was created or changed (exists on disk) or deleted (does not).
    pub fn apply_path(&self, roots: &[Root], path: &Path) {
        let md = std::fs::metadata(path).ok();
        let mut g = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        g.added.retain(|a| a.path != path);
        match md {
            None => {
                g.added.retain(|a| !a.path.starts_with(path));
                g.removed.insert(path.to_path_buf());
            }
            Some(md) => {
                if let Some(a) = make_added(roots, path, &md) {
                    g.added.push(a);
                }
            }
        }
    }
}

fn make_added(roots: &[Root], path: &Path, md: &std::fs::Metadata) -> Option<Added> {
    let root = roots.iter().find(|r| path.starts_with(&r.path))?;
    let rel = path.strip_prefix(&root.path).ok()?;
    let name = rel.file_name()?;
    let ancestors_skipped = rel
        .parent()
        .is_some_and(|p| p.components().any(|c| should_skip(c.as_os_str(), 0, true)));
    if ancestors_skipped || should_skip(name, md.file_attributes(), md.is_dir()) {
        return None;
    }
    let modified_min = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u32::try_from(d.as_secs() / 60).unwrap_or(u32::MAX));
    Some(Added {
        path: path.to_path_buf(),
        name: name.to_string_lossy().into_owned(),
        is_dir: md.is_dir(),
        root: root.id,
        modified_min,
    })
}

/// Owns the OS watchers. Dropping it stops watching.
pub struct WatchHandle {
    _watcher: RecommendedWatcher,
    overlay: Arc<Overlay>,
}

impl WatchHandle {
    /// Start watching every root recursively.
    pub fn start(roots: &[Root]) -> Result<Self, FilesError> {
        let overlay = Arc::new(Overlay::new());
        let (ov, rs) = (Arc::clone(&overlay), roots.to_vec());
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            // Modify(data) is ignored: mtime refreshes on the next scan.
            // ponytail: files inside a directory created or renamed in one go emit no events of their own; the next scan picks them up.
            if matches!(
                ev.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(_))
            ) {
                for p in &ev.paths {
                    ov.apply_path(&rs, p);
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

/// Merge `ov` into `store`, producing a fresh store.
pub(crate) fn fold(store: &FileStore, ov: &Overlay) -> FileStore {
    let g = ov.read();
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
