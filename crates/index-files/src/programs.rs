//! Programs scan: every launchable file (exe, installers, scripts, shortcuts) under a set of roots.
//!
//! ponytail: no live watcher; the list is rescanned at startup (cache gives an instant start).

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
    SKIP_DIRS.iter().any(|s| n.eq_ignore_ascii_case(s))
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
    'roots: for root in roots {
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
            if seen.insert(p.path.to_string_lossy().to_lowercase()) {
                out.push(p);
                if out.len() >= MAX_PROGRAMS {
                    break 'roots;
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.path
            .to_string_lossy()
            .to_lowercase()
            .cmp(&b.path.to_string_lossy().to_lowercase())
    });
    Ok(out)
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
