//! Filesystem watcher keeping the file index current between scans.
//!
//! Changes land in an [`Overlay`] (added entries plus removed paths) that queries merge with
//! the immutable store; [`FileIndex::compact`] folds it back in.

use crate::error::FilesError;
use crate::programs::ProgramFile;
use crate::scan::Root;
use crate::skip::should_skip;
use crate::skiplist::SkipList;
use crate::store::{FileIndex, FileStore, MAX_FILES, NO_PARENT, StoreBuilder};
use notify::event::{ModifyKind, RenameMode};
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
        self.apply_path_with(roots, &SkipList::default(), MAX_FILES, path);
    }

    /// Like [`Overlay::apply_path`], but a path under a directory `skip` excludes is not
    /// added, and at most `room` entries are added (the store's headroom below [`MAX_FILES`]).
    /// Removals are always recorded.
    pub fn apply_path_with(&self, roots: &[Root], skip: &SkipList, room: usize, path: &Path) {
        let md = std::fs::metadata(path).ok();
        let new = md
            .as_ref()
            .map(|m| collect_added(roots, skip, room, path, m));
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
fn collect_added(
    roots: &[Root],
    skip: &SkipList,
    room: usize,
    path: &Path,
    md: &std::fs::Metadata,
) -> Vec<Added> {
    let cap = room.min(WALK_CAP);
    if cap == 0 {
        return Vec::new();
    }
    let Some(first) = make_added(roots, skip, path, md) else {
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
                if skip.excludes_dir(&e.path()) {
                    continue;
                }
                stack.push(e.path());
            }
            out.push(added_of(root, e.path(), &name, &cmd));
            if out.len() >= cap {
                return out; // ponytail: huge moves are truncated; the next scan completes them
            }
        }
    }
    out
}

fn make_added(
    roots: &[Root],
    skip: &SkipList,
    path: &Path,
    md: &std::fs::Metadata,
) -> Option<Added> {
    let root = roots.iter().find(|r| path.starts_with(&r.path))?;
    let rel = path.strip_prefix(&root.path).ok()?;
    let name = rel.file_name()?;
    if should_skip(name, md.file_attributes(), md.is_dir()) {
        return None;
    }
    // A directory is excluded itself; a file only through the folders above it.
    let from = usize::from(!md.is_dir());
    if path.ancestors().skip(from).any(|a| skip.excludes_dir(a)) {
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

/// A program file the watcher saw appear or vanish (D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramChange {
    /// Created, or renamed to this path.
    Added(PathBuf),
    /// Deleted, or renamed away from this path.
    Removed(PathBuf),
}

/// Callback for [`ProgramChange`]s; runs on the notify thread, so it must be quick.
pub type ProgramHook = Arc<dyn Fn(ProgramChange) + Send + Sync>;

/// Program-file changes in one event: a path with a program extension (or a `.app` folder)
/// that no excluded folder above it hides.
fn program_changes(kind: &EventKind, paths: &[PathBuf], skip: &SkipList) -> Vec<ProgramChange> {
    let mut out = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        if ProgramFile::from_path(p).is_none()
            || p.ancestors().skip(1).any(|a| skip.excludes_dir(a))
        {
            continue;
        }
        let added = match kind {
            EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => true,
            EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => false,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => i > 0,
            _ => p.exists(),
        };
        out.push(if added {
            ProgramChange::Added(p.clone())
        } else {
            ProgramChange::Removed(p.clone())
        });
    }
    out
}

/// State the notify callback works on. One place for every event, so later hooks (program
/// extensions, D9) can be added in [`Sink::on_event`].
struct Sink {
    overlay: Arc<Overlay>,
    roots: Vec<Root>,
    skip: SkipList,
    index: Option<Arc<FileIndex>>,
    busy: Arc<AtomicBool>,
    stale: Arc<AtomicBool>,
    on_program: Option<ProgramHook>,
}

impl Sink {
    /// Handle one notify result. Never panics: an error or a rescan notice (events were
    /// lost) only marks the index stale so the next open reconciles.
    fn on_event(&self, res: notify::Result<notify::Event>) {
        let ev = match res {
            Ok(ev) => ev,
            Err(_) => {
                self.stale.store(true, Ordering::Release);
                return;
            }
        };
        if ev.need_rescan() {
            self.stale.store(true, Ordering::Release);
        }
        // Modify(data) is ignored: mtime refreshes on the next scan.
        if !matches!(
            ev.kind,
            EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
        ) {
            return;
        }
        if let Some(hook) = &self.on_program {
            for c in program_changes(&ev.kind, &ev.paths, &self.skip) {
                hook(c);
            }
        }
        for p in &ev.paths {
            let stored = self.index.as_ref().map_or(0, |ix| ix.load().len());
            let room = MAX_FILES.saturating_sub(stored + self.overlay.read().added.len());
            self.overlay
                .apply_path_with(&self.roots, &self.skip, room, p);
        }
        if let Some(ix) = &self.index
            && self.overlay.len() > COMPACT_AT
            && !self.busy.swap(true, Ordering::AcqRel)
        {
            let (ix, ov, flag) = (
                Arc::clone(ix),
                Arc::clone(&self.overlay),
                Arc::clone(&self.busy),
            );
            let spawned = std::thread::Builder::new()
                .name("files-compact".into())
                .spawn(move || {
                    ix.compact(&ov);
                    flag.store(false, Ordering::Release);
                });
            if spawned.is_err() {
                self.busy.store(false, Ordering::Release);
            }
        }
    }
}

/// Owns the OS watchers. Dropping it stops watching.
pub struct WatchHandle {
    _watcher: RecommendedWatcher,
    overlay: Arc<Overlay>,
    stale: Arc<AtomicBool>,
    watched: Vec<Root>,
    failed: Vec<(Root, String)>,
}

impl WatchHandle {
    /// Start watching every root recursively. With `index`, the overlay is compacted into it
    /// on a background thread once it passes [`COMPACT_AT`] entries. `on_program` hears program
    /// file creates and deletes. A root that cannot be
    /// watched (drive gone) is skipped and listed in [`WatchHandle::failed`]; this errors
    /// only when roots were given and none could be watched.
    pub fn start(
        roots: &[Root],
        skip: &SkipList,
        index: Option<Arc<FileIndex>>,
        on_program: Option<ProgramHook>,
    ) -> Result<Self, FilesError> {
        let overlay = Arc::new(Overlay::new());
        let stale = Arc::new(AtomicBool::new(false));
        let sink = Sink {
            overlay: Arc::clone(&overlay),
            roots: roots.to_vec(),
            skip: skip.clone(),
            index,
            busy: Arc::new(AtomicBool::new(false)),
            stale: Arc::clone(&stale),
            on_program,
        };
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            sink.on_event(res);
        })
        .map_err(|e| FilesError::io("create watcher", std::io::Error::other(e)))?;
        let (mut watched, mut failed) = (Vec::new(), Vec::new());
        for r in roots {
            match watcher.watch(&r.path, RecursiveMode::Recursive) {
                Ok(()) => watched.push(r.clone()),
                Err(e) => failed.push((r.clone(), e.to_string())),
            }
        }
        if watched.is_empty()
            && let Some((r, e)) = failed.first()
        {
            return Err(FilesError::io(
                format!("watch {}", r.path.display()),
                std::io::Error::other(e.clone()),
            ));
        }
        Ok(Self {
            _watcher: watcher,
            overlay,
            stale,
            watched,
            failed,
        })
    }

    /// The overlay this watcher feeds.
    pub fn overlay(&self) -> Arc<Overlay> {
        Arc::clone(&self.overlay)
    }

    /// True once a watcher error or a rescan notice means events may have been lost; the
    /// index should be reconciled with a rescan.
    pub fn is_stale(&self) -> bool {
        self.stale.load(Ordering::Acquire)
    }

    /// Roots actually being watched.
    pub fn watched(&self) -> &[Root] {
        &self.watched
    }

    /// Roots that could not be watched, with the reason; callers log these as warnings.
    pub fn failed(&self) -> &[(Root, String)] {
        &self.failed
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

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, Flag};

    fn sink() -> Sink {
        Sink {
            overlay: Arc::new(Overlay::new()),
            roots: Vec::new(),
            skip: SkipList::default(),
            index: None,
            busy: Arc::new(AtomicBool::new(false)),
            stale: Arc::new(AtomicBool::new(false)),
            on_program: None,
        }
    }

    #[test]
    fn error_event_sets_stale() {
        let s = sink();
        s.on_event(Ok(notify::Event::new(EventKind::Create(CreateKind::File))));
        assert!(!s.stale.load(Ordering::Acquire));
        s.on_event(Err(notify::Error::generic("injected")));
        assert!(s.stale.load(Ordering::Acquire));
    }

    #[test]
    fn rescan_notice_sets_stale() {
        let s = sink();
        s.on_event(Ok(
            notify::Event::new(EventKind::Other).set_flag(Flag::Rescan)
        ));
        assert!(s.stale.load(Ordering::Acquire));
    }
}
