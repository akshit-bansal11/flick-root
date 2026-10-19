//! Plain Markdown file target: open, append one line, close. Also the shared line appender
//! used by the Obsidian daily note.

use super::CaptureError;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

fn io(e: std::io::Error) -> CaptureError {
    CaptureError::Io(e.kind().to_string())
}

/// Appends `line` and a newline to `path`, creating the file. Starts a new line first when
/// the file does not end with one. Blocking: call from `spawn_blocking`.
pub fn append_line(path: &Path, line: &str) -> Result<(), CaptureError> {
    let lead = match File::open(path) {
        Ok(mut f) => {
            let len = f.metadata().map_err(io)?.len();
            let mut last = *b"\n";
            if len > 0 {
                f.seek(SeekFrom::End(-1)).map_err(io)?;
                f.read_exact(&mut last).map_err(io)?;
            }
            if last[0] == b'\n' { "" } else { "\n" }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "",
        Err(e) => return Err(io(e)),
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io)?;
    f.write_all(format!("{lead}{line}\n").as_bytes())
        .map_err(io)
}

/// The configured file; its folder must already exist.
fn target(file: &str) -> Result<&Path, CaptureError> {
    let p = Path::new(file.trim());
    if file.trim().is_empty() {
        return Err(CaptureError::NotConfigured("Set a Markdown file"));
    }
    match p.parent() {
        Some(d) if d.as_os_str().is_empty() || d.is_dir() => Ok(p),
        _ => Err(CaptureError::NotConfigured(
            "The Markdown file's folder is missing",
        )),
    }
}

/// Appends `line` to the configured file. Blocking.
pub fn append(file: &str, line: &str) -> Result<(), CaptureError> {
    append_line(target(file)?, line)
}

/// Checks the file can be appended to, without writing. Blocking.
pub fn check(file: &str) -> Result<(), CaptureError> {
    let p = target(file)?;
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() || m.permissions().readonly() => {
            Err(CaptureError::Io("not writable".into()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_fixes_missing_newline() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("inbox.md");
        let s = f.to_string_lossy().into_owned();
        append(&s, "- one").unwrap();
        std::fs::write(&f, "- one").unwrap();
        append(&s, "- two").unwrap();
        append(&s, "- three").unwrap();
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "- one\n- two\n- three\n"
        );
        check(&s).unwrap();
    }

    #[test]
    fn missing_folder_or_unset_is_not_configured() {
        assert!(matches!(
            append("", "x"),
            Err(CaptureError::NotConfigured(_))
        ));
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("nope").join("a.md");
        let r = append(&f.to_string_lossy(), "x");
        assert!(matches!(r, Err(CaptureError::NotConfigured(_))));
        assert!(!d.path().join("nope").exists());
    }
}
