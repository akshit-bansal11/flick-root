//! Programs scan: every launchable file (exe, installers, scripts, shortcuts) under a set of roots.
//!
//! ponytail: no live watcher; the list is rescanned at startup (cache gives an instant start).
//! [`ProgramCache`] (v2) keeps entries per drive root; the flat v1 `save_programs` /
//! `load_programs` stay until the desktop app switches to it.

use crate::error::FilesError;
use crate::scan::Parallelism;
use jwalk::WalkDirGeneric;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Extensions (lowercase, no dot) treated as programs.
pub const PROGRAM_EXTS: &[&str] = &[
    "exe",
    "msi",
    "msix",
    "msixbundle",
    "appx",
    "appxbundle",
    "apk",
    "xapk",
    "aab",
    "bat",
    "cmd",
    "ps1",
    "lnk",
    "url",
    "appref-ms",
    "jar",
    "appinstaller",
    "app",
    "pkg",
    "dmg",
    "appimage",
    "deb",
    "rpm",
    "flatpak",
    "flatpakref",
    "snap",
    "run",
    "sh",
];

/// Hard cap on returned programs; the walk stops when reached.
pub const MAX_PROGRAMS: usize = 50_000;

const CACHE_HEADER: &str = "programs v1";
const CACHE_HEADER_V2: &str = "programs v2";
const ATTR_SYSTEM: u32 = 0x4;
const SKIP_DIRS: &[&str] = &[
    "windows",
    "$recycle.bin",
    "system volume information",
    "$winreagent",
    "$sysreset",
    "recovery",
    "config.msi",
    "winsxs",
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    ".cache",
    "temp",
    "tmp",
    "recent",
    "start menu", // shortcuts there duplicate the app list
    "onedrivetemp",
];

/// A launchable file found by [`scan_programs`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramFile {
    /// Absolute path.
    pub path: PathBuf,
    /// File stem.
    pub name: String,
    /// Lowercase extension without the dot.
    pub ext: String,
}

impl ProgramFile {
    /// Build from a path when its extension is a program extension (case-insensitive).
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if !PROGRAM_EXTS.contains(&ext.as_str()) {
            return None;
        }
        let name = path.file_stem()?.to_string_lossy().into_owned();
        Some(Self {
            path: path.to_path_buf(),
            name,
            ext,
        })
    }
}

fn is_app_bundle(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
}

fn skip_dir_name(name: &OsStr) -> bool {
    let n = name.to_string_lossy();
    // Dot-directories are tool caches (.claude, .npm, .vscode, .gradle, .m2).
    n.starts_with('.') || SKIP_DIRS.iter().any(|s| n.eq_ignore_ascii_case(s))
}

/// Walk one root, pushing new program files (deduplicated through `seen`) into `out` until
/// `out` holds `cap` entries. Returns whether the cap was hit. `cancel` is checked per entry.
fn walk_root(
    root: &Path,
    cap: usize,
    parallelism: &Parallelism,
    cancel: &AtomicBool,
    seen: &mut HashSet<String>,
    out: &mut Vec<ProgramFile>,
) -> Result<bool, FilesError> {
    if out.len() >= cap {
        return Ok(true);
    }
    let walk = WalkDirGeneric::<((), ())>::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .parallelism(parallelism.clone())
        .process_read_dir(|depth, _, _, children| {
            // jwalk first calls this with `depth == None` and the walk root as the only
            // child; keep it even when it is hidden/system (drive roots are).
            if depth.is_none() {
                return;
            }
            // A `.app` directory is a macOS bundle: listed, never descended into.
            for c in children.iter_mut().flatten() {
                if c.file_type.is_dir() && is_app_bundle(&c.file_name) {
                    c.read_children = None;
                }
            }
            children.retain(|c| {
                let Ok(e) = c else { return false };
                let system = e
                    .metadata()
                    .ok()
                    .is_some_and(|m| m.file_attributes() & ATTR_SYSTEM != 0);
                if e.file_type.is_dir() {
                    !system && (is_app_bundle(&e.file_name) || !skip_dir_name(&e.file_name))
                } else {
                    !system
                }
            });
        });
    for item in walk {
        if cancel.load(Ordering::Relaxed) {
            return Err(FilesError::Cancelled); // dropping `walk` stops the workers
        }
        let Ok(e) = item else { continue };
        if !(e.file_type.is_file() || e.file_type.is_dir() && is_app_bundle(&e.file_name)) {
            continue;
        }
        let Some(p) = ProgramFile::from_path(&e.path()) else {
            continue;
        };
        if seen.insert(path_key(&p.path)) {
            out.push(p);
            if out.len() >= cap {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Lowercase path string: the sort and dedup key for program paths.
fn path_key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// Sort by lowercase path and drop case-insensitive duplicates.
fn sort_dedup(list: &mut Vec<ProgramFile>) {
    list.sort_by_cached_key(|p| path_key(&p.path));
    list.dedup_by(|a, b| path_key(&a.path) == path_key(&b.path));
}

/// Walk `roots` and collect program files, sorted by path, deduplicated, capped at [`MAX_PROGRAMS`].
/// `cancel` is checked per entry.
pub fn scan_programs(
    roots: &[PathBuf],
    parallelism: &Parallelism,
    cancel: &AtomicBool,
) -> Result<Vec<ProgramFile>, FilesError> {
    let mut out: Vec<ProgramFile> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for root in roots {
        if walk_root(root, MAX_PROGRAMS, parallelism, cancel, &mut seen, &mut out)? {
            break;
        }
    }
    sort_dedup(&mut out);
    Ok(out)
}

/// Per-root share of [`MAX_PROGRAMS`] when `n_roots` roots are scanned, so one junk drive
/// cannot evict the others.
pub fn root_cap(n_roots: usize) -> usize {
    MAX_PROGRAMS / n_roots.max(1)
}

/// Walk one root with its own cap. Fails (instead of returning an empty list) when the root
/// cannot be listed before or after the walk, so a removed drive never wipes its cached entries.
/// Returns the sorted files and whether the cap was hit.
pub fn scan_root(
    root: &Path,
    cap: usize,
    parallelism: &Parallelism,
    cancel: &AtomicBool,
) -> Result<(Vec<ProgramFile>, bool), FilesError> {
    let gone = |e| FilesError::io(format!("list programs root {}", root.display()), e);
    std::fs::read_dir(root).map_err(gone)?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let capped = walk_root(root, cap, parallelism, cancel, &mut seen, &mut out)?;
    if !root.is_dir() {
        return Err(gone(std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    sort_dedup(&mut out);
    Ok((out, capped))
}

/// One scanned root in a [`ProgramCache`]: its path, volume identity and last scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedRoot {
    /// Root path, normally a drive root such as `C:\`.
    pub root: PathBuf,
    /// Volume serial of the drive when it was scanned.
    pub serial: u32,
    /// Scan completion time, Unix seconds.
    pub scanned: u64,
    /// Whether the walk stopped at its per-root cap.
    pub capped: bool,
}

impl ScannedRoot {
    /// Uppercase drive letter of the root, `None` when the path has no `X:` prefix.
    pub fn letter(&self) -> Option<char> {
        let s = self.root.to_str()?;
        let mut c = s.chars();
        let l = c.next()?;
        (l.is_ascii_alphabetic() && c.next() == Some(':')).then_some(l.to_ascii_uppercase())
    }

    /// Lowercase root prefix ending in a separator, for ownership checks.
    fn prefix(&self) -> String {
        let mut k = path_key(&self.root);
        if !k.ends_with('\\') && !k.ends_with('/') {
            k.push('\\');
        }
        k
    }

    /// Whether `p` lies under this root.
    fn owns(&self, p: &ProgramFile) -> bool {
        path_key(&p.path).starts_with(&self.prefix())
    }
}

/// Current Unix time in seconds (0 if the clock is before 1970).
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Programs cache v2: entries grouped by scanned root, so one drive can be rescanned, or hidden
/// while detached, without touching the others.
/// ponytail: roots must not nest; replacing an outer root drops a nested root's entries too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramCache {
    /// Scanned roots, one per drive.
    pub roots: Vec<ScannedRoot>,
    /// Program files of every root, sorted by lowercase path.
    pub files: Vec<ProgramFile>,
}

impl ProgramCache {
    /// Drop `root`'s entries (and its old record) and insert those of `files` under it, keeping
    /// the sort. Call only after the root's walk completed, so a cancel keeps the old entries.
    pub fn replace_root(&mut self, root: ScannedRoot, files: Vec<ProgramFile>) {
        let key = root.prefix();
        self.roots.retain(|r| r.prefix() != key);
        self.files.retain(|p| !root.owns(p));
        self.files
            .extend(files.into_iter().filter(|p| root.owns(p)));
        sort_dedup(&mut self.files);
        self.roots.push(root);
    }

    /// Keep only the entries for which `f` returns true.
    pub fn retain(&mut self, f: impl FnMut(&ProgramFile) -> bool) {
        self.files.retain(f);
    }

    /// Entries whose root's drive letter and volume serial are in `present`. A detached drive,
    /// or another volume now on the same letter, is hidden but stays cached.
    pub fn visible(&self, present: &[(char, u32)]) -> Vec<ProgramFile> {
        let live: Vec<&ScannedRoot> = self
            .roots
            .iter()
            .filter(|r| r.letter().is_some_and(|l| present.contains(&(l, r.serial))))
            .collect();
        self.files
            .iter()
            .filter(|p| live.iter().any(|r| r.owns(p)))
            .cloned()
            .collect()
    }

    /// Scan each `(root, serial)` with a fair share of [`MAX_PROGRAMS`] and replace it once its
    /// walk completes. A root that fails (drive removed) keeps its old entries and is returned in
    /// the list; a cancel stops with [`FilesError::Cancelled`], keeping roots already finished.
    pub fn scan_into(
        &mut self,
        roots: &[(PathBuf, u32)],
        parallelism: &Parallelism,
        cancel: &AtomicBool,
    ) -> Result<Vec<(PathBuf, FilesError)>, FilesError> {
        let cap = root_cap(roots.len());
        let mut failed = Vec::new();
        for (root, serial) in roots {
            match scan_root(root, cap, parallelism, cancel) {
                Ok((files, capped)) => {
                    let scanned = ScannedRoot {
                        root: root.clone(),
                        serial: *serial,
                        scanned: unix_now(),
                        capped,
                    };
                    self.replace_root(scanned, files);
                }
                Err(FilesError::Cancelled) => return Err(FilesError::Cancelled),
                Err(e) => failed.push((root.clone(), e)),
            }
        }
        Ok(failed)
    }

    /// Write the v2 cache atomically (temp file + rename): header, one
    /// `root <path> <serial hex> <unix secs> <capped 0|1>` line per root, then one path per line.
    pub fn save(&self, path: &Path) -> Result<(), FilesError> {
        let mut text = String::from(CACHE_HEADER_V2);
        for r in &self.roots {
            text.push_str(&format!(
                "\nroot {} {:08x} {} {}",
                r.root.to_string_lossy(),
                r.serial,
                r.scanned,
                u8::from(r.capped)
            ));
        }
        for p in &self.files {
            text.push('\n');
            text.push_str(&p.path.to_string_lossy());
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text).map_err(|e| FilesError::io("write programs cache", e))?;
        std::fs::rename(&tmp, path).map_err(|e| FilesError::io("replace programs cache", e))
    }

    /// Load a v2 cache. `None` when missing, v1, of an unknown version, not UTF-8, or with a
    /// malformed root line, so the caller runs one full scan.
    pub fn load(path: &Path) -> Result<Option<Self>, FilesError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidData
                ) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(FilesError::io("read programs cache", e)),
        };
        let mut lines = text.lines().peekable();
        if lines.next() != Some(CACHE_HEADER_V2) {
            return Ok(None);
        }
        let mut cache = Self::default();
        while let Some(line) = lines.next_if(|l| l.starts_with("root ")) {
            let Some(root) = parse_root(&line["root ".len()..]) else {
                return Ok(None);
            };
            cache.roots.push(root);
        }
        cache.files = lines
            .filter_map(|l| ProgramFile::from_path(Path::new(l)))
            .collect();
        sort_dedup(&mut cache.files);
        Ok(Some(cache))
    }
}

/// Parse `<path> <serial hex> <unix secs> <capped 0|1>`; the path may contain spaces.
fn parse_root(s: &str) -> Option<ScannedRoot> {
    let mut it = s.rsplitn(4, ' ');
    let capped = match it.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let scanned = it.next()?.parse().ok()?;
    let serial = u32::from_str_radix(it.next()?, 16).ok()?;
    let root = it.next().filter(|r| !r.is_empty())?;
    Some(ScannedRoot {
        root: PathBuf::from(root),
        serial,
        scanned,
        capped,
    })
}

/// Write `list` to `path` (header line, then one path per line) via temp file + rename.
pub fn save_programs(path: &Path, list: &[ProgramFile]) -> Result<(), FilesError> {
    let mut text = String::from(CACHE_HEADER);
    for p in list {
        text.push('\n');
        text.push_str(&p.path.to_string_lossy());
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| FilesError::io("write programs cache", e))?;
    std::fs::rename(&tmp, path).map_err(|e| FilesError::io("replace programs cache", e))
}

/// Load a cache written by [`save_programs`]; `None` when missing or of an unknown version.
pub fn load_programs(path: &Path) -> Result<Option<Vec<ProgramFile>>, FilesError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(FilesError::io("read programs cache", e)),
    };
    let mut lines = text.lines();
    if lines.next() != Some(CACHE_HEADER) {
        return Ok(None);
    }
    Ok(Some(
        lines
            .filter_map(|l| ProgramFile::from_path(Path::new(l)))
            .collect(),
    ))
}
