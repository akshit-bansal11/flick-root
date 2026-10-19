//! Drive-root discovery and root resolution for Find mode.

use crate::scan::Root;
use std::path::{Path, PathBuf};

/// Drive letters whose root exists. ponytail: std cannot tell fixed from removable or network
/// drives; callers filter with `win::drives::is_fixed_drive` (crates/win).
pub fn present_drives() -> Vec<char> {
    ('A'..='Z')
        .filter(|c| std::fs::metadata(format!("{c}:\\")).is_ok())
        .collect()
}

/// The system drive letter from `%SystemDrive%` (e.g. `C:`), if set.
pub fn system_drive() -> Option<char> {
    std::env::var("SystemDrive")
        .ok()
        .and_then(|s| s.chars().next())
        .map(|c| c.to_ascii_uppercase())
}

/// Final root list: `user` folders, then (when `include`) one root per drive except `system`,
/// then `extra` paths. Any root lying inside a drive root already in the list is dropped, as
/// are exact duplicates. Ids are reassigned in order; roots past 255 are discarded.
pub fn resolve_roots(
    user: Vec<Root>,
    drives: &[char],
    system: Option<char>,
    include: bool,
    extra: Vec<PathBuf>,
) -> Vec<Root> {
    let drive_roots: Vec<PathBuf> = if include {
        drives
            .iter()
            .map(|d| d.to_ascii_uppercase())
            .filter(|d| Some(*d) != system)
            .map(|d| PathBuf::from(format!("{d}:\\")))
            .collect()
    } else {
        Vec::new()
    };
    let covered = |p: &Path| drive_roots.iter().any(|d| p.starts_with(d));
    let mut out: Vec<Root> = Vec::new();
    let mut push = |path: PathBuf, label: String| {
        if out.iter().any(|r| r.path == path) {
            return;
        }
        if let Ok(id) = u8::try_from(out.len()) {
            out.push(Root { id, path, label });
        }
    };
    for r in user {
        if !covered(&r.path) {
            push(r.path, r.label);
        }
    }
    for d in &drive_roots {
        push(
            d.clone(),
            d.to_string_lossy().trim_end_matches('\\').to_string(),
        );
    }
    for p in extra {
        if !covered(&p) {
            let label = p.file_name().map_or_else(
                || p.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            push(p, label);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(p: &str, l: &str) -> Root {
        Root {
            id: 0,
            path: p.into(),
            label: l.into(),
        }
    }

    fn paths(r: &[Root]) -> Vec<&str> {
        r.iter().filter_map(|r| r.path.to_str()).collect()
    }

    #[test]
    fn adds_non_system_drives() {
        let r = resolve_roots(
            vec![user(r"C:\Users\a\Desktop", "Desktop")],
            &['C', 'D', 'e'],
            Some('C'),
            true,
            vec![],
        );
        assert_eq!(paths(&r), [r"C:\Users\a\Desktop", r"D:\", r"E:\"]);
        assert_eq!(r[2].id, 2);
    }

    #[test]
    fn dedups_nested_and_exact() {
        let r = resolve_roots(
            vec![user(r"D:\Docs", "Documents"), user(r"C:\x", "x")],
            &['D'],
            Some('C'),
            true,
            vec![r"D:\Games".into(), r"C:\x".into(), r"F:\y".into()],
        );
        assert_eq!(paths(&r), [r"C:\x", r"D:\", r"F:\y"]);
    }

    #[test]
    fn disabled_keeps_user_roots() {
        let r = resolve_roots(
            vec![user(r"D:\Docs", "Documents")],
            &['D'],
            Some('C'),
            false,
            vec![],
        );
        assert_eq!(paths(&r), [r"D:\Docs"]);
    }
}
