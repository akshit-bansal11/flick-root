//! Obsidian target: append a line to today's daily note by writing the Markdown file
//! directly (no plugin). Paths never leave the vault.

use super::{CaptureError, markdown};
use chrono::NaiveDate;
use launcher_core::config::Obsidian;
use std::path::{Component, Path, PathBuf};

const DEFAULT_FORMAT: &str = "YYYY-MM-DD";

fn bad(m: &'static str) -> CaptureError {
    CaptureError::NotConfigured(m)
}

/// True for a relative path made only of normal components.
fn is_inside(rel: &str) -> bool {
    Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

/// Daily note file name for `date` from an Obsidian-style format (`YYYY`, `YY`, `MM`, `DD`).
fn file_name(format: &str, date: NaiveDate) -> Result<String, CaptureError> {
    let f = if format.trim().is_empty() {
        DEFAULT_FORMAT
    } else {
        format.trim()
    };
    let chrono_fmt = f
        .replace('%', "%%")
        .replace("YYYY", "%Y")
        .replace("YY", "%y")
        .replace("MM", "%m")
        .replace("DD", "%d");
    let name = date.format(&chrono_fmt).to_string();
    if name.is_empty() || !is_inside(&name) || name.contains(['/', '\\', ':']) {
        return Err(bad("The daily note format is not a file name"));
    }
    Ok(format!("{name}.md"))
}

/// Where the daily note for `date` lives. Checks the vault exists and the folder stays inside it.
pub fn daily_path(cfg: &Obsidian, date: NaiveDate) -> Result<PathBuf, CaptureError> {
    if cfg.vault.trim().is_empty() {
        return Err(bad("Set your Obsidian vault folder"));
    }
    let vault = Path::new(cfg.vault.trim());
    if !vault.is_dir() {
        return Err(bad("The Obsidian vault folder does not exist"));
    }
    let folder = cfg.daily_folder.trim();
    if !is_inside(folder) {
        return Err(bad("The daily note folder must be inside the vault"));
    }
    Ok(vault
        .join(folder)
        .join(file_name(&cfg.daily_note_format, date)?))
}

/// Creates the daily folder (vault exists), confirms it resolves inside the vault, then appends.
/// Blocking.
pub fn append(cfg: &Obsidian, date: NaiveDate, line: &str) -> Result<(), CaptureError> {
    let path = daily_path(cfg, date)?;
    let dir = path.parent().ok_or(bad("No daily note folder"))?;
    let io = |e: std::io::Error| CaptureError::Io(e.kind().to_string());
    let real_vault = Path::new(cfg.vault.trim()).canonicalize().map_err(io)?;
    // Before creating anything: the deepest existing ancestor must already resolve inside the
    // vault, so a linked folder can't make create_dir_all write elsewhere.
    let existing = dir.ancestors().find(|a| a.exists()).unwrap_or(dir);
    if !existing
        .canonicalize()
        .map_err(io)?
        .starts_with(&real_vault)
    {
        return Err(bad("The daily note folder must be inside the vault"));
    }
    std::fs::create_dir_all(dir).map_err(io)?;
    if !dir.canonicalize().map_err(io)?.starts_with(&real_vault) {
        return Err(bad("The daily note folder must be inside the vault"));
    }
    // The note itself must not be a link (symlink or reparse point) out of the vault.
    if let Ok(md) = std::fs::symlink_metadata(&path)
        && (md.file_type().is_symlink()
            || !path.canonicalize().map_err(io)?.starts_with(&real_vault))
    {
        return Err(bad("The daily note is a link outside the vault"));
    }
    markdown::append_line(&path, line)
}

/// Checks the note's folder can be written, without writing. Blocking.
pub fn check(cfg: &Obsidian, today: NaiveDate) -> Result<(), CaptureError> {
    let path = daily_path(cfg, today)?;
    let dir = path.parent().ok_or(bad("No daily note folder"))?;
    let probe = if dir.is_dir() {
        dir
    } else {
        Path::new(cfg.vault.trim())
    };
    match std::fs::metadata(probe) {
        Ok(m) if !m.permissions().readonly() => Ok(()),
        _ => Err(CaptureError::Io("not writable".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
    }

    fn cfg(vault: &Path, folder: &str) -> Obsidian {
        Obsidian {
            vault: vault.to_string_lossy().into_owned(),
            daily_note_format: "YYYY-MM-DD".into(),
            daily_folder: folder.into(),
        }
    }

    #[test]
    fn new_then_existing_note() {
        let v = tempfile::tempdir().unwrap();
        let c = cfg(v.path(), "");
        append(&c, day(), "- 09:00 a").unwrap();
        append(&c, day(), "- 09:01 b").unwrap();
        let t = std::fs::read_to_string(v.path().join("2026-10-19.md")).unwrap();
        assert_eq!(t, "- 09:00 a\n- 09:01 b\n");
        check(&c, day()).unwrap();
    }

    #[test]
    fn daily_folder_is_created_inside_the_vault() {
        let v = tempfile::tempdir().unwrap();
        let mut c = cfg(v.path(), "Daily/2026");
        c.daily_note_format = "YY.MM.DD".into();
        append(&c, day(), "- x").unwrap();
        assert!(v.path().join("Daily/2026/26.10.19.md").is_file());
    }

    #[test]
    fn escapes_are_refused() {
        let v = tempfile::tempdir().unwrap();
        for folder in ["../up", "..", "C:/x", "/abs", "a/../../b"] {
            let r = append(&cfg(v.path(), folder), day(), "- x");
            assert!(matches!(r, Err(CaptureError::NotConfigured(_))), "{folder}");
        }
        let mut c = cfg(v.path(), "");
        c.daily_note_format = "../YYYY".into();
        assert!(append(&c, day(), "- x").is_err());
        assert!(!v.path().parent().unwrap().join("2026.md").exists());
    }

    #[test]
    fn missing_vault_is_not_created() {
        let v = tempfile::tempdir().unwrap();
        let gone = v.path().join("no-vault");
        let r = append(&cfg(&gone, "Daily"), day(), "- x");
        assert!(matches!(r, Err(CaptureError::NotConfigured(_))));
        assert!(!gone.exists());
    }
}
