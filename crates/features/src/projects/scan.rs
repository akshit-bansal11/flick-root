//! Depth-limited walk of the dev roots. Blocking; call from `spawn_blocking`.

use super::markers::{is_project, is_skipped};
use std::collections::HashSet;
use std::path::PathBuf;

/// A found project folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Folder name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
}

/// Finds projects up to `depth` levels below each root. A found project is not entered, so
/// nested projects are not listed. Missing roots are ignored. Returns nothing when `cancelled()`.
pub fn scan(roots: &[PathBuf], depth: usize, cancelled: &dyn Fn() -> bool) -> Vec<Project> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = roots
        .iter()
        .filter(|r| r.is_dir())
        .map(|r| (r.clone(), 0))
        .collect();
    while let Some((dir, level)) = stack.pop() {
        if cancelled() {
            return Vec::new();
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if !ft.is_dir() || is_skipped(&name) {
                continue;
            }
            let path = e.path();
            if is_project(&path) {
                if seen.insert(path.clone()) {
                    out.push(Project { name, path });
                }
            } else if level + 1 < depth {
                stack.push((path, level + 1));
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
