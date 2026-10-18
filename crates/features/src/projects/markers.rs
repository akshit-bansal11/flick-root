//! What makes a folder a project, and which folders a scan never enters.

use std::path::Path;

/// Files or folders whose presence marks a project root.
const MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
];

/// Folder names a scan never descends into (hidden folders are skipped separately).
const SKIPPED: &[&str] = &["node_modules", "target", "dist", "build"];

/// True when `dir` holds one of the project markers.
pub fn is_project(dir: &Path) -> bool {
    MARKERS.iter().any(|m| dir.join(m).exists())
}

/// True when a scan must not enter a folder with this name.
pub fn is_skipped(name: &str) -> bool {
    name.starts_with('.') || SKIPPED.contains(&name)
}
