//! What makes a folder a project, and which folders a scan never enters.

use std::path::Path;

/// Files or folders whose presence marks a project root.
const MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "requirements.txt",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "Gemfile",
    "CMakeLists.txt",
    "Makefile",
    "deno.json",
    "pubspec.yaml",
    "mix.exs",
];

/// File extensions that mark a project root (the file name is not fixed).
const MARKER_EXTENSIONS: &[&str] = &["csproj", "sln", "cabal"];

/// Folder names a scan never descends into (hidden folders are skipped separately).
const SKIPPED: &[&str] = &["node_modules", "target", "dist", "build"];

/// True when `dir` holds one of the project markers. Exact names are checked first; the folder
/// is listed once only when none matched, to look for a marker extension. An unreadable folder
/// is not a project.
pub fn is_project(dir: &Path) -> bool {
    if MARKERS.iter().any(|m| dir.join(m).exists()) {
        return true;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    rd.flatten().any(|e| {
        e.path()
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| MARKER_EXTENSIONS.iter().any(|m| m.eq_ignore_ascii_case(x)))
    })
}

/// True when a scan must not enter a folder with this name.
pub fn is_skipped(name: &str) -> bool {
    name.starts_with('.') || SKIPPED.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::scan::scan;

    fn dir_with(file: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(file), "").unwrap();
        d
    }

    #[test]
    fn every_exact_marker_is_found() {
        for m in MARKERS.iter().filter(|m| **m != ".git") {
            assert!(is_project(dir_with(m).path()), "{m}");
        }
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join(".git")).unwrap();
        assert!(is_project(d.path()));
    }

    #[test]
    fn extension_markers_are_found() {
        for f in ["app.csproj", "x.sln", "y.cabal", "Z.SLN"] {
            assert!(is_project(dir_with(f).path()), "{f}");
        }
    }

    #[test]
    fn non_projects_are_not_found() {
        let d = dir_with("notes.sln.txt");
        std::fs::write(d.path().join("package.jsonx"), "").unwrap();
        assert!(!is_project(d.path()));
        assert!(!is_project(tempfile::tempdir().unwrap().path()));
        assert!(!is_project(&d.path().join("missing")));
    }

    #[test]
    fn a_found_project_is_not_descended_into() {
        let root = tempfile::tempdir().unwrap();
        let outer = root.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(outer.join("Makefile"), "").unwrap();
        std::fs::write(inner.join("app.csproj"), "").unwrap();
        let found = scan(&[root.path().to_path_buf()], 3, &|| false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "outer");
    }
}
