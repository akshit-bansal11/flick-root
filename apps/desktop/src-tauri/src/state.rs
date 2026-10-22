//! Shared application state, startup tasks and shutdown.
use crate::error::CommandError;
use crate::icons::Icons;
use crate::meta_store::JsonMeta;
use anyhow::Context;
use arc_swap::ArcSwap;
use features::media::{Media, spotify::Spotify};
use features::{EventSink, FeatureCtx, Registry, TaskScope, builtin_features_with};
use index_apps::{AppEntry, AppIndex, cache};
use index_files::drives;
use index_files::programs::unix_now;
use index_files::{
    FileIndex, FilesError, Overlay, ProgramCache, ProgramChange, ProgramFile, ProgramHook, Root,
    SkipList, WatchHandle, default_roots, snapshot,
};
use launcher_core::config::{Config, config_path, local_data_dir};
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use launcher_core::store::Store;
use launcher_core::{Input, Item};
use parking_lot::Mutex;
use providers::KeySource;
use providers::hub::ProviderHub;
use providers::secret::{KeyringBackend, SecretVault, VaultKeys};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use win::drives::DriveKind;

/// Apps (Start menu, registry, desktop) are refreshed on open when older than this (seconds).
const APPS_MAX_AGE: u64 = 600;
/// Programs get a full rescan on open when the last full scan is older than this (seconds).
const PROGRAMS_MAX_AGE: u64 = 24 * 3600;
/// The first open of the process reconciles Find when the snapshot is older than this (seconds).
const FIND_MAX_AGE: u64 = 600;
/// Programs cache file in the cache dir.
const PROGRAMS_FILE: &str = "programs.idx";
/// Find snapshot file in the cache dir.
const SNAPSHOT_FILE: &str = "files.idx";

static START: OnceLock<Instant> = OnceLock::new();

/// Records the process start; call first thing in `run`. Later calls are no-ops.
pub fn mark_start() {
    let _ = START.set(Instant::now());
}

/// Logs one startup phase with milliseconds since [`mark_start`].
pub fn phase(name: &str) {
    let ms = START
        .get()
        .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    tracing::info!(phase = name, ms, "startup");
}

/// Real apps and the fuzzy index built over their names; swapped together.
pub struct AppMatch {
    /// Entries, same order as the index.
    pub entries: Arc<Vec<AppEntry>>,
    /// Fuzzy index over entry names.
    pub index: MatchIndex,
}

/// Scanned programs and their own fuzzy index, kept apart so the Command budget never pays for them.
pub struct ProgramSet {
    /// Program file paths, same order as the index. An `AppEntry` is built only for a hit
    /// ([`features::search::program_entry`]).
    pub paths: Vec<Box<str>>,
    /// Per path: it sits on a removable drive (from the scan's drive kinds).
    pub removable: Vec<bool>,
    /// Fuzzy index over the file names.
    pub index: MatchIndex,
}

impl Default for ProgramSet {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            removable: Vec::new(),
            index: MatchIndex::new::<&str>(&[]),
        }
    }
}

/// Cancel flag of the current Find run; starting a run cancels the previous one.
#[derive(Default)]
pub struct FindRun(Mutex<Arc<AtomicBool>>);

impl FindRun {
    /// Cancels the previous run and returns the new run's flag.
    pub fn begin(&self) -> Arc<AtomicBool> {
        let mut cur = self.0.lock();
        cur.store(true, Ordering::Relaxed);
        *cur = Arc::new(AtomicBool::new(false));
        cur.clone()
    }

    /// Cancels the current run.
    pub fn cancel(&self) {
        self.0.lock().store(true, Ordering::Relaxed);
    }

    /// The current run's flag, without cancelling anything.
    pub fn current(&self) -> Arc<AtomicBool> {
        self.0.lock().clone()
    }
}

/// Single-flight flag of one background job: at most one run holds it.
#[derive(Default)]
pub struct Busy(Arc<AtomicBool>);

/// Held while a job runs; releases the job on drop, so a panic cannot wedge it.
pub struct BusyGuard(Arc<AtomicBool>);

impl Busy {
    /// Claims the job; `None` when a run already holds it.
    pub fn try_begin(&self) -> Option<BusyGuard> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| BusyGuard(Arc::clone(&self.0)))
    }

    /// Whether a run holds the job.
    pub fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Claims `busy` and returns the run's cancel flag. A superseding run (settings change, reindex,
/// feature on) cancels the current run and waits for it to let go; an on-open or startup run
/// skips when the job is held. Blocking: call from a blocking thread.
fn acquire(busy: &Busy, run: &FindRun, supersede: bool) -> Option<(BusyGuard, Arc<AtomicBool>)> {
    if !supersede {
        let guard = busy.try_begin()?;
        return Some((guard, run.current()));
    }
    let cancel = run.begin();
    loop {
        if let Some(guard) = busy.try_begin() {
            return Some((guard, cancel));
        }
        if cancel.load(Ordering::Relaxed) {
            return None; // superseded again, or the feature was turned off
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Rescans a skip-list change asked for that no run has completed yet: a newer change cancels the
/// run in flight, and the next run picks these up. A run takes them when it starts and puts them
/// back when it does not complete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingRescan {
    /// Rescan every programs drive (a name pattern was removed).
    pub all: bool,
    /// Programs drives to rescan (a drive or folder entry was removed).
    pub letters: Vec<char>,
    /// Rescan Find completely.
    pub find: bool,
}

impl PendingRescan {
    /// Takes the programs part, leaving it empty.
    fn take_programs(&mut self) -> (bool, Vec<char>) {
        (
            std::mem::take(&mut self.all),
            std::mem::take(&mut self.letters),
        )
    }

    /// Adds `all` and `letters` to the programs part.
    fn add_programs(&mut self, all: bool, letters: &[char]) {
        self.all |= all;
        self.letters.extend_from_slice(letters);
        self.letters.sort_unstable();
        self.letters.dedup();
    }
}

/// When each index was last refreshed, in Unix seconds (0 = never), kept in memory. Seeded from
/// the cache headers and the snapshot mtime at startup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Freshness {
    /// Last apps refresh (or the apps cache mtime).
    pub apps: u64,
    /// Oldest scan among the cached program roots that are present.
    pub programs_full: u64,
    /// `(letter, serial)` of every root in the programs cache, attached or not.
    pub program_roots: Vec<(char, u32)>,
    /// `(letter, serial)` of the drives the published program set was filtered with.
    pub programs_present: Vec<(char, u32)>,
    /// Last Find scan (or the snapshot mtime).
    pub find_snapshot: u64,
    /// The Find watcher reported lost events (filled at plan time).
    pub find_stale: bool,
    /// Drive letters that are roots of the Find store (filled at plan time).
    pub find_drives: Vec<char>,
    /// The launcher has been opened in this process.
    pub opened: bool,
}

/// Settings and job state [`plan_refresh`] decides with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanCfg {
    /// Programs scan on.
    pub programs: bool,
    /// Find mode on.
    pub find: bool,
    /// Find indexes whole drives.
    pub find_drives: bool,
    /// System drive letter (Find covers it through the user folders).
    pub system: Option<char>,
    /// An apps refresh is running.
    pub busy_apps: bool,
    /// A programs run is running.
    pub busy_programs: bool,
    /// A Find run is running.
    pub busy_find: bool,
}

/// Work one launcher open triggers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshPlan {
    /// Refresh the apps list.
    pub apps: bool,
    /// Drive letters whose programs are rescanned (every present drive on a full rescan).
    pub program_roots: Vec<char>,
    /// The rescan is the 24 h full one.
    pub programs_full: bool,
    /// The set of present drives changed: republish (attached shown, detached hidden).
    pub programs_republish: bool,
    /// Rescan Find completely and restart the watcher.
    pub find_reconcile: bool,
    /// Drive roots to add to Find.
    pub find_roots_add: Vec<char>,
    /// Drive roots to drop from Find.
    pub find_roots_drop: Vec<char>,
}

impl RefreshPlan {
    fn programs(&self) -> bool {
        self.programs_full || !self.program_roots.is_empty() || self.programs_republish
    }
}

/// Whether this open settles the first-open Find reconcile: Find is off, or it was not needed. A
/// busy Find job (the startup load) or a reconcile still to run leaves it for a later open, or for
/// the reconcile once it has run.
fn marks_opened(cfg: &PlanCfg, plan: &RefreshPlan) -> bool {
    !cfg.find || !(cfg.busy_find || plan.find_reconcile)
}

/// Decides what one launcher open refreshes. Pure. `drives` are the indexable drives as
/// `(letter, serial, kind)`; `now` is Unix seconds.
pub fn plan_refresh(
    now: u64,
    last: &Freshness,
    drives: &[(char, u32, DriveKind)],
    cfg: &PlanCfg,
) -> RefreshPlan {
    let old = |t: u64, max: u64| t == 0 || now.saturating_sub(t) > max;
    let mut plan = RefreshPlan {
        apps: !cfg.busy_apps && old(last.apps, APPS_MAX_AGE),
        ..RefreshPlan::default()
    };
    if cfg.programs && !cfg.busy_programs {
        let present: Vec<(char, u32)> = drives.iter().map(|&(l, s, _)| (l, s)).collect();
        plan.programs_full = old(last.programs_full, PROGRAMS_MAX_AGE);
        plan.program_roots = present
            .iter()
            .filter(|p| plan.programs_full || !last.program_roots.contains(p))
            .map(|&(l, _)| l)
            .collect();
        plan.programs_republish = present != last.programs_present;
    }
    if cfg.find && !cfg.busy_find {
        plan.find_reconcile =
            last.find_stale || (!last.opened && old(last.find_snapshot, FIND_MAX_AGE));
        if !plan.find_reconcile {
            // Fixed and removable drives, like roots_for.
            let want: Vec<char> = if cfg.find_drives {
                drives
                    .iter()
                    .filter(|d| {
                        matches!(d.2, DriveKind::Fixed | DriveKind::Removable)
                            && Some(d.0) != cfg.system
                    })
                    .map(|d| d.0)
                    .collect()
            } else {
                Vec::new()
            };
            plan.find_roots_add = want
                .iter()
                .filter(|l| !last.find_drives.contains(l))
                .copied()
                .collect();
            plan.find_roots_drop = last
                .find_drives
                .iter()
                .filter(|l| !want.contains(l))
                .copied()
                .collect();
        }
    }
    plan
}

/// What changed between two validated skip lists, compared case-insensitively and ignoring a
/// trailing separator.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkipDiff {
    /// Entries only in the new list.
    pub added: Vec<String>,
    /// Removed drive or folder entries (`E:\`, `D:\games`).
    pub removed_paths: Vec<String>,
    /// Removed name patterns (`node_modules`, `*cache*`, `target\*\deps`).
    pub removed_globs: Vec<String>,
}

impl SkipDiff {
    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed_paths.is_empty() && self.removed_globs.is_empty()
    }
}

fn skip_key(e: &str) -> String {
    e.trim().trim_end_matches(['\\', '/']).to_lowercase()
}

/// Whether `e` starts with a drive (`X:`), i.e. is a drive or a folder path rather than a pattern.
pub fn is_path_entry(e: &str) -> bool {
    let b = e.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Diff of two skip lists. Pure.
pub fn skip_diff(old: &[String], new: &[String]) -> SkipDiff {
    let old_keys: Vec<String> = old.iter().map(|e| skip_key(e)).collect();
    let new_keys: Vec<String> = new.iter().map(|e| skip_key(e)).collect();
    let mut d = SkipDiff::default();
    for (e, k) in new.iter().zip(&new_keys) {
        if !old_keys.contains(k) {
            d.added.push(e.clone());
        }
    }
    for (e, k) in old.iter().zip(&old_keys) {
        if !new_keys.contains(k) {
            if is_path_entry(e) {
                d.removed_paths.push(e.clone());
            } else {
                d.removed_globs.push(e.clone());
            }
        }
    }
    d
}

/// Letters of the whole-drive roots (path `X:\`, label `X:`) among Find's `roots`.
fn drive_letters(roots: &[Root]) -> Vec<char> {
    roots
        .iter()
        .filter_map(|r| {
            let s = r.path.to_str()?;
            let mut c = s.chars();
            let l = c.next()?;
            let whole = s.len() == 3 && c.next() == Some(':') && s.ends_with('\\');
            (whole && r.label.len() == 2).then_some(l.to_ascii_uppercase())
        })
        .collect()
}

/// `(letter, serial)` of every root line at the top of the programs cache (format:
/// [`ProgramCache::save`]) and the oldest of their scans (0 when none or malformed). Reads only
/// those lines, so [`init`] can seed [`Freshness`] without loading the whole cache.
fn cached_program_roots(path: &Path) -> (Vec<(char, u32)>, u64) {
    use std::io::BufRead;
    let Ok(file) = std::fs::File::open(path) else {
        return (Vec::new(), 0);
    };
    let mut roots = Vec::new();
    let mut oldest = u64::MAX;
    for line in std::io::BufReader::new(file)
        .lines()
        .skip(1) // header
        .map_while(Result::ok)
    {
        let Some(rest) = line.strip_prefix("root ") else {
            break;
        };
        // `<path> <serial hex> <unix secs> <capped>`; split from the right, the path may hold spaces.
        let mut f = rest.rsplitn(4, ' ').skip(1);
        let secs = f.next().and_then(|v| v.parse::<u64>().ok());
        let serial = f.next().and_then(|v| u32::from_str_radix(v, 16).ok());
        let (Some(secs), Some(serial)) = (secs, serial) else {
            return (Vec::new(), 0); // malformed: the cache loads as missing too
        };
        let mut c = f.next().unwrap_or_default().chars();
        if let (Some(l), Some(':')) = (c.next(), c.next())
            && l.is_ascii_alphabetic()
        {
            roots.push((l.to_ascii_uppercase(), serial));
            oldest = oldest.min(secs);
        }
    }
    if roots.is_empty() {
        return (Vec::new(), 0);
    }
    (roots, oldest)
}

/// Modification time of `path` in Unix seconds, 0 when unreadable.
fn mtime_secs(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Everything commands and tasks share.
pub struct AppState {
    /// Live config.
    pub config: Arc<ArcSwap<Config>>,
    /// SQLite store.
    pub store: Arc<Store>,
    /// Installed apps.
    pub apps: Arc<AppIndex>,
    /// Scanned programs plus their fuzzy index; empty while `apps.scan_programs` is off.
    pub programs: ArcSwap<ProgramSet>,
    /// Cancel flag of the programs scan (off, a new scan, or exit).
    pub programs_run: FindRun,
    programs_gate: Mutex<()>,
    /// Program changes the Find watcher saw, waiting for the next coalesced apply (D9).
    program_pending: Mutex<Vec<ProgramChange>>,
    /// A delayed apply is queued; false while idle, so no timer runs.
    program_scheduled: AtomicBool,
    /// Skip-list rescans no run has completed yet.
    rescan: Mutex<PendingRescan>,
    /// An attached drive's programs walk stopped at its cap (for the settings page).
    pub programs_capped: AtomicBool,
    /// Single-flight flags of the apps, programs and Find jobs and of the on-open refresh.
    apps_busy: Busy,
    programs_busy: Busy,
    find_busy: Busy,
    refresh_busy: Busy,
    /// When each index was last refreshed.
    fresh: Mutex<Freshness>,
    /// Apps plus fuzzy index.
    pub app_match: ArcSwap<AppMatch>,
    /// Reused matcher scratch for apps and features (keystroke thread).
    pub matcher: Mutex<FuzzyMatcher>,
    /// Matcher scratch for the programs match (blocking pool).
    pub prog_matcher: Mutex<FuzzyMatcher>,
    /// File index.
    pub files: Arc<FileIndex>,
    /// Live file changes.
    pub overlay: ArcSwap<Overlay>,
    /// File watcher, held while Find is on.
    pub watch: Mutex<Option<WatchHandle>>,
    /// Write-only secret vault (Credential Manager plus metadata).
    pub vault: Arc<SecretVault>,
    /// Cancels the running Find scan (on Find off, a new run, or exit).
    pub find_run: FindRun,
    /// Optional Spotify connection (also feeds the media feature).
    pub spotify: Arc<Spotify>,
    /// Feature registry.
    pub registry: Arc<Registry>,
    /// Handles given to features.
    pub ctx: FeatureCtx,
    /// App icons.
    pub icons: Icons,
    /// Items of the latest search, by id.
    pub last_results: Mutex<HashMap<String, Item>>,
    /// Input of the latest search.
    pub last_input: Mutex<Option<Input>>,
    /// Latest search sequence number.
    pub latest_seq: AtomicU64,
    /// Key source over the vault, for building providers.
    pub keys: Arc<dyn KeySource>,
    config_path: PathBuf,
    config_write: Mutex<()>,
    cache_dir: PathBuf,
}

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        if let Err(err) = self.0.emit(name, payload) {
            tracing::warn!(%err, name, "emit failed");
        }
    }
}

/// Builds the fuzzy index for the real apps `entries`.
pub fn build_match(entries: Arc<Vec<AppEntry>>) -> AppMatch {
    let names: Vec<&str> = entries.iter().map(|a| a.name.as_str()).collect();
    let index = MatchIndex::new(&names);
    AppMatch { entries, index }
}

fn is_shortcut(p: &ProgramFile) -> bool {
    matches!(p.ext.as_str(), "lnk" | "url")
}

/// Program paths, minus any an app already launches and any shortcut named like a real app
/// (Start Menu and Desktop shortcuts to apps already listed).
pub fn merge_programs(apps: &[AppEntry], programs: &[ProgramFile]) -> Vec<Box<str>> {
    let mut seen: std::collections::HashSet<String> = apps
        .iter()
        .flat_map(|a| {
            let t = a
                .target
                .as_ref()
                .map(|t| t.to_string_lossy().to_lowercase());
            [Some(a.parsing_name.to_lowercase()), t]
        })
        .flatten()
        .collect();
    let app_names: std::collections::HashSet<String> =
        apps.iter().map(|a| a.name.to_lowercase()).collect();
    let mut out = Vec::with_capacity(programs.len());
    for p in programs {
        if is_shortcut(p) && app_names.contains(&p.name.to_lowercase()) {
            continue;
        }
        let path = p.path.to_string_lossy().into_owned();
        if !seen.insert(path.to_lowercase()) {
            continue;
        }
        out.push(path.into_boxed_str());
    }
    out.shrink_to_fit();
    out
}

/// [`merge_programs`] plus the fuzzy index. `removable_letters` are the drive letters whose
/// `DriveKind` is `Removable` (from the drive scan); an entry is removable when its path starts there.
pub fn build_programs(
    apps: &[AppEntry],
    programs: &[ProgramFile],
    removable_letters: &[char],
) -> ProgramSet {
    let paths = merge_programs(apps, programs);
    let mut removable: Vec<bool> = paths
        .iter()
        .map(|p| {
            let mut c = p.chars();
            matches!((c.next(), c.next()), (Some(l), Some(':'))
                if removable_letters.contains(&l.to_ascii_uppercase()))
        })
        .collect();
    removable.shrink_to_fit();
    let index = features::search::program_index(&paths);
    ProgramSet {
        paths,
        removable,
        index,
    }
}

/// Lowercase `p` ending in one separator, for "lies under" checks.
fn dir_key(p: &Path) -> String {
    let mut k = p.to_string_lossy().to_lowercase();
    if !k.ends_with('\\') {
        k.push('\\');
    }
    k
}

/// Applies watcher `changes` in order to `cache`, matching paths case-insensitively: an added
/// program file under a cached root joins it; a removed path drops that file and every file under
/// it (a deleted or renamed-away folder). Keeps the cache sorted by lowercase path. Returns
/// whether anything changed. Pure; the watcher feeds it (D9).
pub fn apply_cache_changes(cache: &mut ProgramCache, changes: &[ProgramChange]) -> bool {
    let key = |p: &Path| p.to_string_lossy().to_lowercase();
    let roots: Vec<String> = cache.roots.iter().map(|r| dir_key(&r.root)).collect();
    let mut list: Vec<(String, ProgramFile)> = std::mem::take(&mut cache.files)
        .into_iter()
        .map(|f| (key(&f.path), f))
        .collect();
    let mut changed = false;
    for c in changes {
        match c {
            ProgramChange::Removed(p) => {
                let (k, dir) = (key(p), dir_key(p));
                let n = list.len();
                list.retain(|(f, _)| *f != k && !f.starts_with(&dir));
                changed |= list.len() != n;
            }
            ProgramChange::Added(p) => {
                let k = key(p);
                if roots.iter().any(|r| k.starts_with(r))
                    && !list.iter().any(|(f, _)| *f == k)
                    && let Some(f) = ProgramFile::from_path(p)
                {
                    list.push((k, f));
                    changed = true;
                }
            }
        }
    }
    if changed {
        list.sort_by(|a, b| a.0.cmp(&b.0));
    }
    cache.files = list.into_iter().map(|(_, f)| f).collect();
    changed
}

/// Whether a path in `sorted` lies under the folder `dir`. `sorted` is ordered by lowercase path,
/// as the published program set is (cache order, which [`merge_programs`] keeps).
fn has_program_under(sorted: &[Box<str>], dir: &Path) -> bool {
    let k = dir_key(dir);
    let i = sorted.partition_point(|p| p.to_lowercase() < k);
    sorted
        .get(i)
        .is_some_and(|p| p.to_lowercase().starts_with(&k))
}

/// The Find watcher's program hook. Holds the state weakly so the watcher never keeps it alive.
fn program_hook(st: &Arc<AppState>) -> ProgramHook {
    let weak = Arc::downgrade(st);
    Arc::new(move |change| {
        if let Some(st) = weak.upgrade() {
            queue_program_change(&st, change);
        }
    })
}

/// Queues `change`; the first one of a burst schedules a single apply 2 s later, retried every
/// 2 s while a programs run holds the job. A no-op (and the queue is cleared) while Programs is
/// off. Runs on the notify thread: a removed non-program path (maybe a folder) is queued only when
/// a listed program lies under it, a binary search.
fn queue_program_change(st: &Arc<AppState>, change: ProgramChange) {
    if !st.config.load().apps.scan_programs {
        st.program_pending.lock().clear();
        return;
    }
    if let ProgramChange::Removed(p) = &change
        && ProgramFile::from_path(p).is_none()
        && !has_program_under(&st.programs.load().paths, p)
    {
        return;
    }
    st.program_pending.lock().push(change);
    if st.program_scheduled.swap(true, Ordering::AcqRel) {
        return;
    }
    let s = st.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let st = s.clone();
            match tauri::async_runtime::spawn_blocking(move || flush_program_changes(&st)).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(err) => {
                    tracing::warn!(%err, "program changes task failed");
                    s.program_scheduled.store(false, Ordering::Release);
                    break;
                }
            }
        }
    });
}

/// Applies the queued changes to the programs cache, saves it and publishes, holding the programs
/// job so it never races a scan or a skip filter. Returns true (retry later) when a programs run
/// holds the job; that run merges the queue itself. Blocking (cache I/O, drive query, index build).
fn flush_program_changes(st: &AppState) -> bool {
    if !st.config.load().apps.scan_programs {
        st.program_pending.lock().clear();
        st.program_scheduled.store(false, Ordering::Release);
        return false;
    }
    let Some(_busy) = st.programs_busy.try_begin() else {
        return true;
    };
    // Cleared before draining: an event after this schedules a fresh apply.
    st.program_scheduled.store(false, Ordering::Release);
    let changes = std::mem::take(&mut *st.program_pending.lock());
    if changes.is_empty() {
        return false;
    }
    let path = st.programs_path();
    let mut cache = match ProgramCache::load(&path) {
        Ok(Some(c)) => c,
        Ok(None) => return false, // nothing cached yet: the first run scans everything
        Err(err) => {
            tracing::warn!(%err, "programs cache unreadable; changes wait for the next scan");
            return false;
        }
    };
    if apply_cache_changes(&mut cache, &changes) {
        if let Err(err) = cache.save(&path) {
            tracing::warn!(%err, "saving programs cache failed");
        }
        let (present, removable) = drive_sets(&win::drives::indexable_drives());
        publish_programs(st, &cache, &present, &removable, || {
            st.config.load().apps.scan_programs
        });
    }
    false
}

/// `(letter, serial)` of the indexable drives, and the letters of the removable ones.
fn drive_sets(vols: &[win::drives::DriveRoot]) -> (Vec<(char, u32)>, Vec<char>) {
    let present = vols.iter().map(|d| (d.letter, d.serial)).collect();
    let removable = vols
        .iter()
        .filter(|d| d.kind == DriveKind::Removable)
        .map(|d| d.letter)
        .collect();
    (present, removable)
}

/// Builds the program set from `cache`'s entries on the `present` drives and publishes it under
/// the gate if `live()` still holds there. [`stop_programs`] cancels before taking the gate, so a
/// store here either lands first (and is cleared) or sees the flag. Blocking.
fn publish_programs(
    st: &AppState,
    cache: &ProgramCache,
    present: &[(char, u32)],
    removable: &[char],
    live: impl Fn() -> bool,
) {
    let set = build_programs(&st.apps.entries(), &cache.visible(present), removable);
    let _gate = st.programs_gate.lock();
    if live() {
        st.programs.store(Arc::new(set));
    }
}

impl AppState {
    /// Applies `f` to a copy of the config, saves it atomically, publishes it and
    /// rebuilds the provider hub. Writers are serialized; `f` errors change nothing.
    pub fn update_config<R>(
        &self,
        f: impl FnOnce(&mut Config) -> Result<R, CommandError>,
    ) -> Result<R, CommandError> {
        let _guard = self.config_write.lock();
        let mut next = (*self.config.load_full()).clone();
        let out = f(&mut next)?;
        next.save(&self.config_path)
            .map_err(|e| CommandError::new("store_failed", e.to_string()))?;
        self.config.store(Arc::new(next));
        self.rebuild_hub();
        Ok(out)
    }

    /// Rebuilds the provider hub from the live config.
    pub fn rebuild_hub(&self) {
        let hub = ProviderHub::from_config(&self.config.load().providers, &self.keys);
        self.ctx.providers.store(Arc::new(hub));
    }

    fn rebuild_match(&self) {
        self.app_match
            .store(Arc::new(build_match(self.apps.entries())));
    }

    fn programs_path(&self) -> PathBuf {
        self.cache_dir.join(PROGRAMS_FILE)
    }

    fn snapshot_path(&self) -> PathBuf {
        self.cache_dir.join(SNAPSHOT_FILE)
    }

    fn save_snapshot(&self) {
        if self.files.is_empty() {
            return;
        }
        if let Err(err) = snapshot::save(&self.snapshot_path(), &self.files.load()) {
            tracing::warn!(%err, "saving file snapshot failed");
        }
    }
}

fn roots_for(cfg: &Config) -> Vec<Root> {
    let find = &cfg.modes.find;
    let user: Vec<Root> = default_roots()
        .into_iter()
        .filter(|r| find.roots.iter().any(|n| n.eq_ignore_ascii_case(&r.label)))
        .collect();
    // Runs on the blocking pool: GetDriveTypeW can stall on network or optical drives.
    // Fixed and removable drives; network and optical stay out. MAX_FILES bounds the store.
    let drive_list: Vec<char> = if find.include_drives {
        win::drives::indexable_drives()
            .into_iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    win::drives::DriveKind::Fixed | win::drives::DriveKind::Removable
                )
            })
            .map(|d| d.letter)
            .collect()
    } else {
        Vec::new()
    };
    let extra = find
        .extra_roots
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    drives::resolve_roots(
        user,
        &drive_list,
        drives::system_drive(),
        find.include_drives,
        extra,
    )
}

/// Creates and manages [`AppState`]. Cheap: no scanning here.
pub fn init(app: &tauri::App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let config_path = config_path().context("locating config")?;
    let config = Arc::new(ArcSwap::from_pointee(
        Config::load(&config_path).unwrap_or_else(|err| {
            tracing::warn!(%err, "config unreadable, using defaults");
            Config::default()
        }),
    ));
    phase("config loaded");
    let data = local_data_dir().context("locating data dir")?;
    std::fs::create_dir_all(&data).context("creating data dir")?;
    let (store, report) = Store::open(&data.join("store.db")).context("opening store")?;
    let store = Arc::new(store);
    phase("store opened");
    if report.reset
        && let Err(err) = handle.emit("store://reset", serde_json::json!({}))
    {
        tracing::warn!(%err, "emit failed");
    }
    let cache_dir = cache::cache_dir().context("locating cache dir")?;
    std::fs::create_dir_all(&cache_dir).context("creating cache dir")?;
    let apps = Arc::new(AppIndex::new());
    let apps_loaded = apps.load_cache(&cache_dir).unwrap_or_else(|err| {
        tracing::warn!(%err, "app cache unreadable");
        false
    });
    let apps_stamp = if apps_loaded {
        mtime_secs(&cache_dir.join("apps.json")).max(1)
    } else {
        0
    };
    // Seeded from the cache headers, so an open before the startup runs finish plans no walk.
    let (program_roots, programs_full) = cached_program_roots(&cache_dir.join(PROGRAMS_FILE));
    let find_snapshot = mtime_secs(&cache_dir.join(SNAPSHOT_FILE));
    phase("app cache loaded");
    let vault = Arc::new(SecretVault::new(
        Arc::new(KeyringBackend),
        Arc::new(JsonMeta::open(data.join("secret_meta.json"))),
    ));
    let keys: Arc<dyn KeySource> = Arc::new(VaultKeys(vault.clone()));
    let spotify =
        Arc::new(Spotify::new(vault.clone(), config.clone()).context("building spotify client")?);
    let lost = handle.clone();
    spotify.on_lost(move || {
        let payload = serde_json::json!({ "connected": false, "name": serde_json::Value::Null });
        if let Err(err) = lost.emit("spotify://state", payload) {
            tracing::warn!(%err, "emit failed");
        }
    });
    let hub = ProviderHub::from_config(&config.load().providers, &keys);
    let ctx = FeatureCtx {
        config: config.clone(),
        store: store.clone(),
        providers: Arc::new(ArcSwap::from_pointee(hub)),
        http: reqwest::Client::builder()
            .build()
            .context("building http client")?,
        events: Arc::new(TauriSink(handle)),
        tasks: TaskScope::new(),
    };
    let state = Arc::new(AppState {
        config,
        store,
        app_match: ArcSwap::from_pointee(build_match(apps.entries())),
        programs: ArcSwap::from_pointee(ProgramSet::default()),
        programs_run: FindRun::default(),
        programs_gate: Mutex::new(()),
        program_pending: Mutex::new(Vec::new()),
        program_scheduled: AtomicBool::new(false),
        rescan: Mutex::new(PendingRescan::default()),
        programs_capped: AtomicBool::new(false),
        apps_busy: Busy::default(),
        programs_busy: Busy::default(),
        find_busy: Busy::default(),
        refresh_busy: Busy::default(),
        fresh: Mutex::new(Freshness {
            apps: apps_stamp,
            programs_full,
            program_roots,
            find_snapshot,
            ..Freshness::default()
        }),
        prog_matcher: Mutex::new(FuzzyMatcher::new()),
        apps,
        matcher: Mutex::new(FuzzyMatcher::new()),
        files: Arc::new(FileIndex::new()),
        overlay: ArcSwap::from_pointee(Overlay::new()),
        watch: Mutex::new(None),
        vault,
        spotify,
        find_run: FindRun::default(),
        registry: Arc::new(Registry::new()),
        ctx,
        icons: Icons::new(index_apps::icons::IconCache::new(&cache_dir)),
        last_results: Mutex::new(HashMap::new()),
        last_input: Mutex::new(None),
        latest_seq: AtomicU64::new(0),
        keys,
        config_path,
        config_write: Mutex::new(()),
        cache_dir,
    });
    app.manage(state);
    phase("state init done");
    Ok(())
}

/// Spawns background startup work; returns immediately. Caches are published without walks; an
/// index is built at startup only when it has no cache (first run), serially: apps, Find, programs.
pub fn start(app: &AppHandle) {
    let handle = app.clone();
    let state: Arc<AppState> = Arc::clone(&*app.state::<Arc<AppState>>());
    let s = state.clone();
    tauri::async_runtime::spawn(async move {
        for f in builtin_features_with(
            Media::with_api(s.spotify.clone()),
            features::capture::Capture::new(s.vault.clone()),
        ) {
            s.registry.register(f).await;
        }
        let host = Arc::new(crate::nlhost::ShellHost(Arc::downgrade(&s)));
        s.registry
            .register(Arc::new(features::NlFind::new(host)))
            .await;
        let cfg = s.config.load_full();
        s.registry.apply_config(&cfg, &s.ctx).await;
        if let Err(err) = crate::hotkey::sync_explain(&handle, &cfg.general.explain_hotkey) {
            tracing::warn!(%err.message, "explain hotkey not bound");
        }
    });
    let s = state;
    tauri::async_runtime::spawn(async move {
        let first_run = s.fresh.lock().apps == 0;
        if first_run {
            refresh_apps(&s).await;
        } else {
            warm_icons(&s).await;
        }
        if s.config.load().modes.find.enabled {
            load_find(&s).await;
        }
        run_programs(&s, false, Vec::new(), false).await;
    });
}

/// Reads the cached app icons into memory (no extraction, no walk).
async fn warm_icons(s: &Arc<AppState>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        st.icons.warm(&st.apps.entries(), crate::icons::ICON_PX);
    })
    .await;
    if let Err(err) = res {
        tracing::warn!(%err, "icon warm task failed");
    }
}

/// Called after the launcher is shown: spawns the on-open refresh and returns at once, so the
/// show path never waits on it. One refresh at a time; an open during a refresh adds nothing.
pub fn on_open(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let s = Arc::clone(&*state);
    crate::update::check_if_stale(app);
    tauri::async_runtime::spawn(async move { refresh_on_open(&s).await });
}

async fn refresh_on_open(s: &Arc<AppState>) {
    let Some(_run) = s.refresh_busy.try_begin() else {
        return;
    };
    let st = s.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || {
        // A few syscalls per letter; never touches remote or optical drives (D6).
        let vols: Vec<(char, u32, DriveKind)> = win::drives::indexable_drives()
            .into_iter()
            .map(|d| (d.letter, d.serial, d.kind))
            .collect();
        let c = st.config.load();
        let cfg = PlanCfg {
            programs: c.apps.scan_programs,
            find: c.modes.find.enabled,
            find_drives: c.modes.find.include_drives,
            system: drives::system_drive(),
            busy_apps: st.apps_busy.is_busy(),
            busy_programs: st.programs_busy.is_busy(),
            busy_find: st.find_busy.is_busy(),
        };
        let stale = st.watch.lock().as_ref().is_some_and(WatchHandle::is_stale);
        let find_drives = drive_letters(st.files.load().roots());
        let mut f = st.fresh.lock();
        f.find_stale = stale;
        f.find_drives = find_drives;
        let plan = plan_refresh(unix_now(), &f, &vols, &cfg);
        if marks_opened(&cfg, &plan) {
            f.opened = true;
        }
        plan
    })
    .await;
    let plan = match plan {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(%err, "refresh plan task failed");
            return;
        }
    };
    // Serial, like startup: one walk at a time.
    if plan.apps {
        refresh_apps(s).await;
    }
    if plan.find_reconcile {
        if find_full(s, false).await {
            s.fresh.lock().opened = true;
        }
    } else if !plan.find_roots_add.is_empty() || !plan.find_roots_drop.is_empty() {
        find_drive_roots(s, plan.find_roots_add.clone(), plan.find_roots_drop.clone()).await;
    }
    if plan.programs() {
        run_programs(s, plan.programs_full, plan.program_roots, false).await;
    }
}

/// Refreshes the app list and extracts the icons missing on disk. Skips when one is running.
pub(crate) async fn refresh_apps(s: &Arc<AppState>) {
    let Some(busy) = s.apps_busy.try_begin() else {
        tracing::debug!("apps refresh already running");
        return;
    };
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        let stats = st.apps.refresh_blocking()?;
        st.apps.save_cache(&st.cache_dir)?;
        st.rebuild_match();
        // Icons after the list is usable; ensure skips files that exist.
        let _com = index_apps::com::ComGuard::new().map_err(index_apps::AppsError::Enumerate)?;
        let entries = st.apps.entries();
        for e in entries.iter() {
            if let Err(err) = st.icons.cache().ensure(e) {
                tracing::debug!(%err, app = %e.name, "icon extraction failed");
            }
        }
        st.icons.warm(&entries, crate::icons::ICON_PX);
        Ok::<_, index_apps::AppsError>(stats)
    })
    .await;
    // Stamped on failure too, so a broken source is retried in 10 min, not on every open.
    s.fresh.lock().apps = unix_now();
    match res {
        Ok(Ok(stats)) => tracing::info!(kept = stats.kept, "apps refreshed"),
        Ok(Err(err)) => tracing::warn!(%err, "app refresh failed"),
        Err(err) => tracing::warn!(%err, "app refresh task failed"),
    }
}

/// Loads the programs cache, scans what is asked, saves and publishes the entries of the drives
/// that are attached. Scans every drive when `all` or when there is no cache, else only the drive
/// letters in `roots`, plus any skip-list rescan still pending. Off when `apps.scan_programs` is
/// false; [`stop_programs`] cancels it. `supersede` (settings, reindex) cancels a running run;
/// otherwise a running run makes this skip.
pub(crate) async fn run_programs(s: &Arc<AppState>, all: bool, roots: Vec<char>, supersede: bool) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((busy, cancel)) = acquire(&st.programs_busy, &st.programs_run, supersede) else {
            return Err(FilesError::Cancelled);
        };
        let (p_all, p_letters) = st.rescan.lock().take_programs();
        let mut roots = roots;
        roots.extend_from_slice(&p_letters);
        let out = programs_job(&st, all || p_all, &roots, supersede, &cancel);
        if out.is_err() {
            // Put back before the job is released, so the next run picks them up.
            st.rescan.lock().add_programs(p_all, &p_letters);
        }
        drop(busy);
        out
    })
    .await;
    match res {
        Ok(Ok((Some((roots, ms)), count))) => tracing::info!(roots, count, ms, "programs scanned"),
        Ok(Ok((None, _))) => {}
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("programs run cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "programs scan failed"),
        Err(err) => tracing::warn!(%err, "programs scan task failed"),
    }
}

/// Body of [`run_programs`], run while holding the programs job. Returns the scanned root count
/// and time (when it walked) and the cached entry count. Blocking.
fn programs_job(
    st: &AppState,
    all: bool,
    roots: &[char],
    supersede: bool,
    cancel: &AtomicBool,
) -> Result<(Option<(usize, u64)>, usize), FilesError> {
    let live = || !cancel.load(Ordering::Relaxed) && st.config.load().apps.scan_programs;
    if !live() {
        return Err(FilesError::Cancelled);
    }
    let vols = win::drives::indexable_drives();
    let (present, removable) = drive_sets(&vols);
    let path = st.programs_path();
    let (mut cache, cached) = match ProgramCache::load(&path) {
        Ok(Some(c)) => (c, true),
        Ok(None) => (ProgramCache::default(), false),
        Err(err) => {
            tracing::warn!(%err, "programs cache unreadable");
            (ProgramCache::default(), false)
        }
    };
    let targets: Vec<(PathBuf, u32)> = vols
        .iter()
        .filter(|d| all || !cached || roots.contains(&d.letter))
        .map(|d| (PathBuf::from(format!("{}:\\", d.letter)), d.serial))
        .collect();
    let mut scanned = None;
    if !targets.is_empty() {
        if supersede && cached {
            publish_programs(st, &cache, &present, &removable, live); // usable while it rescans
        }
        let started = Instant::now();
        let failed = {
            let pool = scan_pool();
            let _bg = win::thread::BackgroundGuard::new();
            let skip = SkipList::new(&st.config.load().apps.skip);
            cache.scan_into(&targets, vols.len(), &pool, cancel, &skip)?
        };
        for (root, err) in &failed {
            tracing::warn!(root = %root.display(), %err, "programs root not scanned");
        }
        if !live() {
            return Err(FilesError::Cancelled);
        }
        scanned = Some((targets.len(), started.elapsed().as_millis() as u64));
    }
    // Watcher changes queued meanwhile join the cache before it is saved and published; the
    // flush retries while this run holds the job, so none is lost or published over.
    let changes = std::mem::take(&mut *st.program_pending.lock());
    let merged = apply_cache_changes(&mut cache, &changes);
    if (scanned.is_some() || merged)
        && let Err(err) = cache.save(&path)
    {
        tracing::warn!(%err, "saving programs cache failed");
    }
    publish_programs(st, &cache, &present, &removable, live);
    let oldest = cache
        .roots
        .iter()
        .filter(|r| r.letter().is_some_and(|l| present.contains(&(l, r.serial))))
        .map(|r| r.scanned)
        .min()
        .unwrap_or(0);
    let program_roots = cache
        .roots
        .iter()
        .filter_map(|r| Some((r.letter()?, r.serial)))
        .collect();
    let count = cache.files.len();
    st.programs_capped.store(
        cache
            .roots
            .iter()
            .any(|r| r.capped && r.letter().is_some_and(|l| present.iter().any(|p| p.0 == l))),
        Ordering::Relaxed,
    );
    drop(cache); // the scanned list is not needed once published
    let mut f = st.fresh.lock();
    f.programs_full = oldest;
    f.program_roots = program_roots;
    f.programs_present = present;
    Ok((scanned, count))
}

/// Programs off: cancels a running scan and frees the program entries and index.
pub(crate) fn stop_programs(s: &AppState) {
    s.programs_run.cancel();
    let _gate = s.programs_gate.lock();
    s.program_pending.lock().clear();
    s.rescan.lock().take_programs();
    s.programs.store(Arc::new(ProgramSet::default()));
    s.programs_capped.store(false, Ordering::Relaxed);
}

/// Drops from the cached programs every file under a folder the current skip list excludes, and
/// saves. Uses the whole current list (idempotent), so a superseded apply loses nothing.
/// Blocking; returns how many entries went.
fn filter_programs(st: &AppState) -> Result<usize, FilesError> {
    let Some((_busy, _cancel)) = acquire(&st.programs_busy, &st.programs_run, true) else {
        return Err(FilesError::Cancelled);
    };
    let path = st.programs_path();
    let Some(mut cache) = ProgramCache::load(&path)? else {
        return Ok(0);
    };
    let skip = SkipList::new(&st.config.load().apps.skip);
    let before = cache.files.len();
    cache.retain(|p| !p.path.ancestors().skip(1).any(|a| skip.excludes_dir(a)));
    let gone = before - cache.files.len();
    if gone > 0 {
        cache.save(&path)?;
    }
    Ok(gone)
}

/// Restarts the Find watcher with the current skip list (so removed entries stop filtering it and
/// added ones start) and drops from the store what the `added` entries exclude: drive and folder
/// entries by prefix, name patterns by a pass over the store's folders. No walk. Publishes only
/// over the store it started from. Returns whether a Find run was in flight, which may have read
/// the old list. Blocking.
fn filter_find(st: &Arc<AppState>, added: &[String]) -> bool {
    let mut watch = st.watch.lock();
    // stop_find cancels, then takes this lock: checking under it leaves no window.
    if !st.config.load().modes.find.enabled {
        return false;
    }
    let old = watch.take();
    if let Some(h) = &old {
        // Fold pending watcher changes in first: they carry the current root ids.
        st.files.compact(&h.overlay());
    }
    let cur = st.files.load();
    let mut next: Option<index_files::FileStore> = None;
    for e in added.iter().filter(|e| is_path_entry(e)) {
        next = Some(next.as_ref().unwrap_or(&*cur).without_prefix(Path::new(e)));
    }
    let globs: Vec<String> = added
        .iter()
        .filter(|e| !is_path_entry(e))
        .cloned()
        .collect();
    if !globs.is_empty() {
        let skip = SkipList::new(&globs);
        next = Some(
            next.as_ref()
                .unwrap_or(&*cur)
                .without_dirs(|p| skip.excludes_dir(p)),
        );
    }
    if let Some(n) = next {
        if st.files.replace_if(&cur, n) {
            st.save_snapshot();
        } else {
            tracing::debug!("find store changed meanwhile; exclusion filter dropped");
        }
    }
    if old.is_some() {
        let roots = st.files.load().roots().to_vec();
        if let Err(err) = watch_roots(st, &mut watch, &roots) {
            tracing::warn!(%err, "find watcher failed");
        }
    }
    st.find_busy.is_busy()
}

/// Applies a skip-list change: added entries filter the cached programs and the Find store with no
/// walk, and the Find watcher restarts with the new list. A removed drive or folder rescans only
/// its programs drive, a removed pattern every programs drive, and any removal rescans Find, all
/// at background priority. A newer change cancels the runs in flight; their rescans stay pending
/// (in [`AppState`]) until a run completes them.
/// ponytail: a removed entry rescans its whole drive (programs) or all of Find, not only that
/// folder; per-folder rescans when that walk costs too much.
pub(crate) async fn apply_skip(s: &Arc<AppState>, diff: SkipDiff) {
    if diff.is_empty() {
        return;
    }
    let all = !diff.removed_globs.is_empty();
    let mut letters: Vec<char> = diff
        .removed_paths
        .iter()
        .filter_map(|e| e.chars().next().map(|c| c.to_ascii_uppercase()))
        .collect();
    letters.sort_unstable();
    letters.dedup();
    let removed = all || !letters.is_empty();
    {
        let mut p = s.rescan.lock();
        if s.config.load().apps.scan_programs {
            p.add_programs(all, &letters);
        }
        if removed && s.config.load().modes.find.enabled {
            p.find = true;
        }
    }
    let st = s.clone();
    let added = diff.added.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let find_busy = filter_find(&st, &added);
        let gone = if added.is_empty() {
            Ok(0)
        } else {
            filter_programs(&st)
        };
        (find_busy, gone)
    })
    .await;
    let mut find_busy = false;
    match res {
        Ok((busy, gone)) => {
            find_busy = busy;
            match gone {
                Ok(gone) => tracing::info!(gone, "programs filtered by new exclusions"),
                Err(FilesError::Cancelled) => tracing::debug!("exclusion filter superseded"),
                Err(err) => tracing::warn!(%err, "exclusion filter failed"),
            }
        }
        Err(err) => tracing::warn!(%err, "exclusion filter task failed"),
    }
    if s.config.load().apps.scan_programs && (removed || !diff.added.is_empty()) {
        // With nothing to rescan this only republishes the filtered cache.
        run_programs(s, all, letters, true).await;
    }
    // A Find run in flight may have read the old list; a pending rescan may have been cancelled.
    let pending = s.rescan.lock().find;
    if s.config.load().modes.find.enabled && (removed || find_busy || pending) {
        find_full(s, true).await;
    }
}

/// Two background-mode workers for one run. The pool is dropped with the returned value when the
/// run ends or is cancelled, so no scan thread outlives its run.
fn scan_pool() -> index_files::Parallelism {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .thread_name(|i| format!("find-scan-{i}"))
        .start_handler(|_| {
            if let Err(err) = win::thread::enter_background_mode() {
                tracing::debug!(%err, "scan worker background mode unavailable");
            }
        })
        .build();
    match pool {
        Ok(pool) => index_files::Parallelism::RayonExistingPool {
            pool: Arc::new(pool),
            busy_timeout: None,
        },
        Err(err) => {
            tracing::warn!(%err, "scan pool failed; scanning serially");
            index_files::Parallelism::Serial
        }
    }
}

/// Starts watching `roots` and publishes the watcher's overlay. Roots that cannot be watched
/// (drive gone) are logged and skipped.
fn watch_roots(
    st: &Arc<AppState>,
    watch: &mut Option<WatchHandle>,
    roots: &[Root],
) -> Result<(), FilesError> {
    *watch = None; // the old watcher stops before the new one starts
    let skip = SkipList::new(&st.config.load().apps.skip);
    let handle = st.files.watch_auto(roots, &skip, Some(program_hook(st)))?;
    for (root, why) in handle.failed() {
        tracing::warn!(root = %root.path.display(), why, "find root not watched");
    }
    st.overlay.store(handle.overlay());
    *watch = Some(handle);
    Ok(())
}

/// Startup with Find on: publishes the snapshot and watches its roots, with no walk. Without a
/// snapshot (first run) it scans once.
async fn load_find(s: &Arc<AppState>) {
    let st = s.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.find_busy, &st.find_run, false) else {
            return true; // another run builds the index
        };
        let store = match snapshot::load(&st.snapshot_path()) {
            Ok(Some(store)) => store,
            Ok(None) => return false,
            Err(err) => {
                tracing::warn!(%err, "file snapshot unreadable");
                return false;
            }
        };
        // stop_find cancels, then takes this lock: checking under it leaves no window.
        let mut watch = st.watch.lock();
        if cancel.load(Ordering::Relaxed) || !st.config.load().modes.find.enabled {
            return true;
        }
        let roots = store.roots().to_vec();
        st.files.replace(store);
        if let Err(err) = watch_roots(&st, &mut watch, &roots) {
            tracing::warn!(%err, "find watcher failed");
        }
        drop(watch);
        st.fresh.lock().find_snapshot = mtime_secs(&st.snapshot_path()).max(1);
        true
    })
    .await
    .unwrap_or_else(|err| {
        tracing::warn!(%err, "find load task failed");
        true
    });
    if !loaded {
        find_full(s, false).await;
    }
}

/// Full Find scan (Find on, a roots change or a reindex): supersedes a running Find run.
pub(crate) async fn start_find(s: &Arc<AppState>) {
    find_full(s, true).await;
}

/// Scans every Find root, publishes and saves the store and restarts the watcher. Until the scan
/// ends, an empty index shows the snapshot. Takes a pending skip-list rescan, putting it back when
/// it does not complete. Returns whether it completed.
async fn find_full(s: &Arc<AppState>, supersede: bool) -> bool {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((busy, cancel)) = acquire(&st.find_busy, &st.find_run, supersede) else {
            return Err(FilesError::Cancelled);
        };
        let took = std::mem::take(&mut st.rescan.lock().find);
        let out = find_full_job(&st, &cancel);
        if out.is_err() && took {
            // Put back before the job is released, so the next run picks it up.
            st.rescan.lock().find = true;
        }
        drop(busy);
        out
    })
    .await;
    let done = matches!(res, Ok(Ok(())));
    match res {
        Ok(Ok(())) => tracing::info!("find index ready"),
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("find run cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "find index failed"),
        Err(err) => tracing::warn!(%err, "find task failed"),
    }
    done
}

/// Body of [`find_full`], run while holding the Find job. Blocking.
fn find_full_job(st: &Arc<AppState>, cancel: &AtomicBool) -> Result<(), FilesError> {
    let live = || !cancel.load(Ordering::Relaxed) && st.config.load().modes.find.enabled;
    if !live() {
        return Err(FilesError::Cancelled);
    }
    if st.files.is_empty() {
        match snapshot::load(&st.snapshot_path()) {
            Ok(Some(store)) => {
                // Check and replace under the watch lock stop_find takes after cancelling.
                let _watch = st.watch.lock();
                if live() {
                    st.files.replace(store);
                }
            }
            Ok(None) => {}
            Err(err) => tracing::warn!(%err, "file snapshot unreadable"),
        }
    }
    let roots = roots_for(&st.config.load());
    let started = Instant::now();
    let store = {
        let pool = scan_pool();
        let _bg = win::thread::BackgroundGuard::new();
        let skip = SkipList::new(&st.config.load().apps.skip);
        let (store, capped) =
            index_files::scan_capped(&roots, &skip, index_files::MAX_FILES, &pool, cancel, |_| {})?;
        if capped {
            tracing::warn!(
                max = index_files::MAX_FILES,
                "find scan stopped at the entry cap; the index is incomplete"
            );
        }
        store
    };
    tracing::info!(
        files = store.len(),
        roots = roots.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "find scan finished"
    );
    // stop_find cancels, then takes this lock: checking under it leaves no window.
    let mut watch = st.watch.lock();
    if !live() {
        return Err(FilesError::Cancelled);
    }
    st.files.replace(store);
    st.save_snapshot();
    watch_roots(st, &mut watch, &roots)?;
    drop(watch);
    st.fresh.lock().find_snapshot = unix_now();
    Ok(())
}

/// Adds the drive roots `add` (one walk each) and drops `drop_roots` (no I/O), keeping the rest
/// of the store, then saves the snapshot and restarts the watcher on the new roots.
async fn find_drive_roots(s: &Arc<AppState>, add: Vec<char>, drop_roots: Vec<char>) {
    let st = s.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let Some((_busy, cancel)) = acquire(&st.find_busy, &st.find_run, false) else {
            return Err(FilesError::Cancelled);
        };
        let live = || !cancel.load(Ordering::Relaxed) && st.config.load().modes.find.enabled;
        let mut scanned = Vec::new();
        if !add.is_empty() {
            let pool = scan_pool();
            let _bg = win::thread::BackgroundGuard::new();
            for l in &add {
                let root = Root {
                    id: 0,
                    path: PathBuf::from(format!("{l}:\\")),
                    label: format!("{l}:"),
                };
                let skip = SkipList::new(&st.config.load().apps.skip);
                let (store, capped) = index_files::scan_capped(
                    std::slice::from_ref(&root),
                    &skip,
                    index_files::MAX_FILES,
                    &pool,
                    &cancel,
                    |_| {},
                )?;
                if capped {
                    tracing::warn!(drive = %l, "find scan stopped at the entry cap");
                }
                scanned.push((root, store));
            }
        }
        let mut watch = st.watch.lock();
        if !live() {
            return Err(FilesError::Cancelled);
        }
        // Fold pending watcher changes in before the store is rebuilt from it.
        if let Some(h) = watch.take() {
            st.files.compact(&h.overlay());
        }
        let cur = st.files.load();
        let mut next: Option<index_files::FileStore> = None;
        for l in &drop_roots {
            let n = next
                .as_ref()
                .unwrap_or(&*cur)
                .without_root(std::path::Path::new(&format!("{l}:\\")));
            next = Some(n);
        }
        for (root, store) in &scanned {
            let added = next.as_ref().unwrap_or(&*cur).with_root(root, store);
            match added {
                Ok(n) => next = Some(n),
                Err(err) => tracing::warn!(%err, "find drive root not added"),
            }
        }
        if let Some(n) = next {
            st.files.replace(n);
            st.save_snapshot();
        }
        let roots = st.files.load().roots().to_vec();
        watch_roots(&st, &mut watch, &roots)?;
        Ok((add.len(), drop_roots.len()))
    })
    .await;
    match res {
        Ok(Ok((added, dropped))) => tracing::info!(added, dropped, "find drive roots updated"),
        Ok(Err(FilesError::Cancelled)) => tracing::debug!("find drive update cancelled or skipped"),
        Ok(Err(err)) => tracing::warn!(%err, "find drive update failed"),
        Err(err) => tracing::warn!(%err, "find drive task failed"),
    }
}

/// Find off: drops the watcher and frees the file index memory.
pub(crate) fn stop_find(s: &AppState) {
    s.find_run.cancel();
    s.watch.lock().take();
    s.rescan.lock().find = false;
    s.overlay.store(Arc::new(Overlay::new()));
    s.files
        .replace(index_files::store::StoreBuilder::new().finish(Vec::new()));
}

/// Stops features, flushes the store and saves the file snapshot.
pub fn shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    state.find_run.cancel();
    state.programs_run.cancel();
    state.watch.lock().take();
    tauri::async_runtime::block_on(state.registry.stop_all());
    state.store.shutdown();
    if let Err(err) = state.vault.flush() {
        tracing::warn!(%err, "secret metadata flush failed");
    }
    state.save_snapshot();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn entry(name: &str, parsing: &str, target: Option<&str>) -> AppEntry {
        AppEntry {
            id: name.into(),
            name: name.into(),
            parsing_name: parsing.into(),
            target: target.map(PathBuf::from),
            is_packaged: false,
        }
    }

    fn prog(path: &str) -> ProgramFile {
        ProgramFile::from_path(Path::new(path)).unwrap()
    }

    #[test]
    fn merge_drops_duplicates_and_app_shortcuts() {
        let apps = vec![
            entry("A", "shell.A", Some(r"C:\x\a.exe")),
            entry("B", r"C:\x\b.lnk", None),
            entry("Notepad", "Microsoft.Notepad!App", None),
        ];
        let progs = vec![
            prog(r"C:\X\A.EXE"),
            prog(r"C:\x\b.lnk"),
            prog(r"C:\x\c.msi"),
            prog(r"C:\x\c.msi"),
            prog(r"C:\Users\me\Desktop\notepad.LNK"), // shortcut named like an app
            prog(r"C:\tools\notepad.exe"),            // same name but a real exe: kept
        ];
        let out = merge_programs(&apps, &progs);
        let kept: Vec<&str> = out.iter().map(|p| &**p).collect();
        assert_eq!(kept, [r"C:\x\c.msi", r"C:\tools\notepad.exe"]);
        // The entry built for a hit keeps the id, parsing name and admin gate it had before.
        let e = features::search::program_entry(&out[0]);
        assert_eq!(e.name, "c");
        assert_eq!(e.parsing_name, r"C:\x\c.msi");
        assert!(e.target.is_none());
        assert_eq!(e.id, index_apps::make_id(r"C:\x\c.msi"));
        assert!(features::search::can_run_as_admin(&e, true));
    }

    fn root(letter: char, serial: u32, scanned: u64) -> index_files::ScannedRoot {
        index_files::ScannedRoot {
            root: PathBuf::from(format!("{letter}:\\")),
            serial,
            scanned,
            capped: false,
        }
    }

    fn cache_paths(cache: &ProgramCache) -> Vec<String> {
        cache
            .files
            .iter()
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn program_changes_apply_deduped_and_case_insensitive() {
        let mut cache = ProgramCache::default();
        cache.replace_root(
            root('C', 1, 1),
            vec![prog(r"C:\a\Old.exe"), prog(r"C:\a\keep.exe")],
        );
        let changes = [
            ProgramChange::Removed(r"c:\A\OLD.EXE".into()),
            ProgramChange::Added(r"C:\b\new.exe".into()),
            ProgramChange::Added(r"c:\B\NEW.exe".into()),
            ProgramChange::Added(r"C:\b\readme.txt".into()),
            ProgramChange::Added(r"Q:\no\root.exe".into()), // no cached root owns it
        ];
        assert!(apply_cache_changes(&mut cache, &changes));
        assert_eq!(cache_paths(&cache), [r"C:\a\keep.exe", r"C:\b\new.exe"]);
        assert!(!apply_cache_changes(&mut cache, &[]), "no change, no save");
    }

    #[test]
    fn hook_changes_survive_a_concurrent_rescan() {
        // The rescan's result replaces C:; changes queued during it are merged before publishing.
        let mut cache = ProgramCache::default();
        cache.replace_root(root('C', 1, 1), vec![prog(r"C:\old\gone.exe")]);
        cache.replace_root(
            root('C', 1, 2),
            vec![prog(r"C:\scan\found.exe"), prog(r"C:\scan\deleted.exe")],
        );
        let queued = [
            ProgramChange::Added(r"C:\new\installed.exe".into()),
            ProgramChange::Removed(r"C:\scan\deleted.exe".into()),
        ];
        apply_cache_changes(&mut cache, &queued);
        let set = build_programs(&[], &cache.visible(&[('C', 1)]), &[]);
        let got: Vec<&str> = set.paths.iter().map(|p| &**p).collect();
        assert_eq!(got, [r"C:\new\installed.exe", r"C:\scan\found.exe"]);
    }

    #[test]
    fn folder_delete_removes_nested_programs() {
        let mut cache = ProgramCache::default();
        cache.replace_root(
            root('C', 1, 1),
            vec![
                prog(r"C:\Tools\a.exe"),
                prog(r"C:\Tools\sub\b.exe"),
                prog(r"C:\ToolsX\c.exe"), // sibling with a shared name prefix stays
            ],
        );
        assert!(apply_cache_changes(
            &mut cache,
            &[ProgramChange::Removed(r"c:\tools".into())]
        ));
        assert_eq!(cache_paths(&cache), [r"C:\ToolsX\c.exe"]);
        let published: Vec<Box<str>> = vec![r"C:\Tools\sub\b.exe".into(), r"D:\x.exe".into()];
        assert!(has_program_under(&published, Path::new(r"c:\TOOLS")));
        assert!(has_program_under(&published, Path::new(r"C:\Tools\sub\")));
        assert!(!has_program_under(&published, Path::new(r"C:\Tool")));
        assert!(!has_program_under(&published, Path::new(r"E:\")));
    }

    #[test]
    fn cached_program_roots_reads_the_root_lines() {
        let mut cache = ProgramCache::default();
        cache.replace_root(root('C', 0xC0, 500), vec![prog(r"C:\a.exe")]);
        cache.replace_root(root('d', 0xD0, 300), vec![prog(r"D:\b.exe")]);
        let dir = std::env::temp_dir().join(format!("flick-roots-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(PROGRAMS_FILE);
        cache.save(&path).unwrap();
        let (roots, oldest) = cached_program_roots(&path);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(roots, [('C', 0xC0), ('D', 0xD0)]);
        assert_eq!(oldest, 300);
        assert_eq!(cached_program_roots(&dir.join("missing")), (Vec::new(), 0));
    }

    #[test]
    fn pending_rescan_is_put_back_and_unioned() {
        let mut p = PendingRescan::default();
        p.add_programs(false, &['D']);
        let (all, letters) = p.take_programs();
        assert_eq!(p, PendingRescan::default());
        p.add_programs(false, &['E', 'D']); // a newer change while the run was in flight
        p.add_programs(all, &letters); // that run was cancelled: put back
        assert_eq!(p.letters, ['D', 'E']);
        assert!(!p.all);
        p.add_programs(true, &[]);
        assert!(p.all);
    }

    #[test]
    fn removable_flag_follows_drive_kind() {
        let progs = vec![prog(r"E:\a.exe"), prog(r"c:\b.exe"), prog(r"e:\c.exe")];
        let set = build_programs(&[], &progs, &['E']);
        assert_eq!(set.removable, [true, false, true]);
        assert_eq!(set.removable.len(), set.paths.len());
    }

    #[test]
    fn empty_program_set_is_empty() {
        let set = ProgramSet::default();
        assert!(set.paths.is_empty() && set.index.is_empty());
    }

    #[test]
    fn begin_cancels_previous_run() {
        let run = FindRun::default();
        let a = run.begin();
        assert!(!a.load(Ordering::Relaxed));
        let b = run.begin();
        assert!(a.load(Ordering::Relaxed));
        assert!(!b.load(Ordering::Relaxed));
        run.cancel();
        assert!(b.load(Ordering::Relaxed));
        assert!(run.current().load(Ordering::Relaxed));
    }

    const NOW: u64 = 1_000_000;
    const MIN: u64 = 60;
    const C: (char, u32, DriveKind) = ('C', 0xC0, DriveKind::Fixed);
    const D: (char, u32, DriveKind) = ('D', 0xD0, DriveKind::Fixed);
    const E: (char, u32, DriveKind) = ('E', 0xE0, DriveKind::Removable);

    fn cfg() -> PlanCfg {
        PlanCfg {
            programs: true,
            find: true,
            find_drives: true,
            system: Some('C'),
            ..PlanCfg::default()
        }
    }

    /// Everything refreshed a minute ago with C: and D: attached, after the first open.
    fn fresh() -> Freshness {
        Freshness {
            apps: NOW - MIN,
            programs_full: NOW - MIN,
            program_roots: vec![('C', 0xC0), ('D', 0xD0)],
            programs_present: vec![('C', 0xC0), ('D', 0xD0)],
            find_snapshot: NOW - MIN,
            find_stale: false,
            find_drives: vec!['D'],
            opened: true,
        }
    }

    #[test]
    fn plan_refresh_table() {
        let both = [C, D];
        let cases = vec![
            (
                "fresh caches",
                fresh(),
                both.to_vec(),
                cfg(),
                RefreshPlan::default(),
            ),
            (
                "apps 11 min old",
                Freshness {
                    apps: NOW - 11 * MIN,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    apps: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "new removable volume",
                fresh(),
                vec![C, D, E],
                cfg(),
                RefreshPlan {
                    program_roots: vec!['E'],
                    programs_republish: true,
                    find_roots_add: vec!['E'],
                    ..RefreshPlan::default()
                },
            ),
            (
                "same letter, new serial",
                fresh(),
                vec![C, ('D', 0xD1, DriveKind::Fixed)],
                cfg(),
                RefreshPlan {
                    program_roots: vec!['D'],
                    programs_republish: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "detached volume",
                fresh(),
                vec![C],
                cfg(),
                RefreshPlan {
                    programs_republish: true,
                    find_roots_drop: vec!['D'],
                    ..RefreshPlan::default()
                },
            ),
            (
                "programs 25 h old",
                Freshness {
                    programs_full: NOW - 25 * 3600,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    program_roots: vec!['C', 'D'],
                    programs_full: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "first open, 1 h old snapshot",
                Freshness {
                    find_snapshot: NOW - 60 * MIN,
                    opened: false,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    find_reconcile: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "later open, 1 h old snapshot",
                Freshness {
                    find_snapshot: NOW - 60 * MIN,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan::default(),
            ),
            (
                "stale watcher",
                Freshness {
                    find_stale: true,
                    ..fresh()
                },
                both.to_vec(),
                cfg(),
                RefreshPlan {
                    find_reconcile: true,
                    ..RefreshPlan::default()
                },
            ),
            (
                "jobs already running",
                Freshness {
                    apps: 0,
                    programs_full: 0,
                    find_stale: true,
                    ..fresh()
                },
                vec![C, E],
                PlanCfg {
                    busy_apps: true,
                    busy_programs: true,
                    busy_find: true,
                    ..cfg()
                },
                RefreshPlan::default(),
            ),
            (
                "features off",
                Freshness {
                    find_stale: true,
                    ..fresh()
                },
                vec![C, E],
                PlanCfg {
                    programs: false,
                    find: false,
                    ..cfg()
                },
                RefreshPlan::default(),
            ),
            (
                "include_drives turned off drops drive roots",
                fresh(),
                both.to_vec(),
                PlanCfg {
                    find_drives: false,
                    ..cfg()
                },
                RefreshPlan {
                    find_roots_drop: vec!['D'],
                    ..RefreshPlan::default()
                },
            ),
        ];
        for (name, last, drives, cfg, want) in cases {
            assert_eq!(plan_refresh(NOW, &last, &drives, &cfg), want, "{name}");
        }
    }

    #[test]
    fn new_fixed_drive_joins_find() {
        let plan = plan_refresh(
            NOW,
            &fresh(),
            &[C, D, ('F', 0xF0, DriveKind::Fixed)],
            &cfg(),
        );
        assert_eq!(plan.find_roots_add, ['F']);
        assert_eq!(plan.program_roots, ['F']);
        assert!(plan.find_roots_drop.is_empty());
    }

    #[test]
    fn init_seeding_plans_no_full_walk() {
        // As init seeds it from valid caches: nothing published yet, not opened.
        let seeded = Freshness {
            apps: NOW - MIN,
            programs_full: NOW - MIN,
            program_roots: vec![('C', 0xC0), ('D', 0xD0)],
            find_snapshot: NOW - MIN,
            find_drives: vec!['D'],
            ..Freshness::default()
        };
        let plan = plan_refresh(NOW, &seeded, &[C, D], &cfg());
        assert_eq!(
            plan,
            RefreshPlan {
                programs_republish: true, // publishes the cache, no walk
                ..RefreshPlan::default()
            }
        );
        // Unseeded, the same open walked every drive and reconciled Find.
        let bare = Freshness {
            apps: NOW - MIN,
            find_drives: vec!['D'],
            ..Freshness::default()
        };
        let plan = plan_refresh(NOW, &bare, &[C, D], &cfg());
        assert!(plan.programs_full && plan.find_reconcile);
    }

    #[test]
    fn first_open_waits_for_a_reconcile_that_ran() {
        let reconcile = RefreshPlan {
            find_reconcile: true,
            ..RefreshPlan::default()
        };
        let busy = PlanCfg {
            busy_find: true,
            ..cfg()
        };
        let off = PlanCfg {
            find: false,
            ..cfg()
        };
        assert!(marks_opened(&cfg(), &RefreshPlan::default()), "not needed");
        assert!(!marks_opened(&busy, &RefreshPlan::default()), "find busy");
        assert!(!marks_opened(&cfg(), &reconcile), "set once it ran");
        assert!(marks_opened(&off, &RefreshPlan::default()), "find off");
    }

    #[test]
    fn busy_is_single_flight_and_panic_safe() {
        let busy = Busy::default();
        let guard = busy.try_begin();
        assert!(guard.is_some() && busy.is_busy());
        assert!(busy.try_begin().is_none());
        drop(guard);
        assert!(!busy.is_busy());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = busy.try_begin();
            panic!("job failed");
        }));
        assert!(caught.is_err());
        assert!(busy.try_begin().is_some());
    }

    #[test]
    fn acquire_skips_when_held_and_supersede_waits() {
        let busy = Busy::default();
        let run = FindRun::default();
        let held = acquire(&busy, &run, false);
        assert!(held.is_some());
        assert!(acquire(&busy, &run, false).is_none());
        // A superseding run cancels the holder's flag, then claims the job once it lets go.
        let (guard, first) = held.unwrap();
        let busy = Arc::new(busy);
        let run = Arc::new(run);
        let (b, r) = (busy.clone(), run.clone());
        let next = std::thread::spawn(move || acquire(&b, &r, true).map(|(_, c)| c));
        while !first.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        drop(guard);
        let cancel = next.join().unwrap().unwrap();
        assert!(!cancel.load(Ordering::Relaxed));
        assert!(!busy.is_busy(), "the superseding run's guard was dropped");
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn skip_diff_table() {
        let cases = [
            ("same", vec!["a"], vec!["a"], SkipDiff::default()),
            (
                "case and trailing separator are the same entry",
                vec![r"E:\", r"D:\Games"],
                vec!["e:", r"d:\games\"],
                SkipDiff::default(),
            ),
            (
                "added",
                vec!["node_modules"],
                vec!["node_modules", r"E:\", "*cache*"],
                SkipDiff {
                    added: strs(&[r"E:\", "*cache*"]),
                    ..SkipDiff::default()
                },
            ),
            (
                "removed drive, folder and glob",
                vec![r"E:\", r"D:\games", "node_modules", r"target\*\deps"],
                vec![],
                SkipDiff {
                    removed_paths: strs(&[r"E:\", r"D:\games"]),
                    removed_globs: strs(&["node_modules", r"target\*\deps"]),
                    ..SkipDiff::default()
                },
            ),
            (
                "swap",
                vec!["recent"],
                vec![r"F:\x"],
                SkipDiff {
                    added: strs(&[r"F:\x"]),
                    removed_globs: strs(&["recent"]),
                    ..SkipDiff::default()
                },
            ),
        ];
        for (name, old, new, want) in cases {
            assert_eq!(skip_diff(&strs(&old), &strs(&new)), want, "{name}");
        }
        assert!(SkipDiff::default().is_empty());
    }

    #[test]
    fn drive_letters_picks_whole_drive_roots() {
        let root = |p: &str, l: &str| Root {
            id: 0,
            path: p.into(),
            label: l.into(),
        };
        let roots = [
            root(r"C:\Users\me\Desktop", "Desktop"),
            root(r"D:\", "D:"),
            root(r"E:\", r"E:\"), // an extra root that happens to be a drive
        ];
        assert_eq!(drive_letters(&roots), ['D']);
    }
}
