//! VS Code "recently opened" folders, read from its `state.vscdb` (read-only).

use percent_encoding::percent_decode_str;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

const KEY: &str = "history.recentlyOpenedPathsList";

/// `%APPDATA%\Code\User\globalStorage\state.vscdb`, when `APPDATA` is set.
pub fn default_db() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(r"Code\User\globalStorage\state.vscdb"))
}

/// Recent local folders, newest first. A missing or unreadable DB, or a missing key, gives an
/// empty list. Blocking: call from `spawn_blocking`.
pub fn recent_folders(db: &Path) -> Vec<PathBuf> {
    if !db.is_file() {
        return Vec::new();
    }
    match read(db) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "could not read VS Code recents");
            Vec::new()
        }
    }
}

fn read(db: &Path) -> rusqlite::Result<Vec<PathBuf>> {
    // READ_ONLY never takes a write lock, so a running VS Code is not disturbed.
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let value: Option<String> = conn
        .query_row("SELECT value FROM ItemTable WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })?;
    let Some(json) = value.and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok()) else {
        return Ok(Vec::new());
    };
    Ok(json
        .get("entries")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("folderUri")?.as_str())
        .filter_map(uri_to_path)
        .collect())
}

/// `file:///c%3A/Users/me/app` to `c:\Users\me\app`. Remote, UNC and non-drive URIs give `None`.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file:///")?;
    let decoded = percent_decode_str(rest).decode_utf8().ok()?;
    let b = decoded.as_bytes();
    if b.len() < 2 || !b[0].is_ascii_alphabetic() || b[1] != b':' {
        return None;
    }
    Some(PathBuf::from(decoded.replace('/', "\\")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uris() {
        assert_eq!(
            uri_to_path("file:///c%3A/Users/me/my%20app%20%26%20x"),
            Some(PathBuf::from("c:\\Users\\me\\my app & x"))
        );
        assert_eq!(uri_to_path("vscode-remote://ssh-remote%2Bh/home/x"), None);
        assert_eq!(uri_to_path("file://server/share/x"), None);
        assert_eq!(uri_to_path("file:///home/x"), None);
    }

    #[test]
    fn reads_fixture_db() {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("state.vscdb");
        let json = serde_json::json!({ "entries": [
            { "folderUri": "file:///d%3A/dev/alpha" },
            { "folderUri": "vscode-remote://wsl%2Bubuntu/home/x" },
            { "fileUri": "file:///d%3A/dev/file.txt" },
            { "workspace": { "id": "1", "configPath": "file:///d%3A/dev/w.code-workspace" } },
            { "folderUri": "file:///e%3A/beta%20two" },
        ]});
        {
            let c = Connection::open(&db).unwrap();
            c.execute("CREATE TABLE ItemTable (key TEXT UNIQUE, value BLOB)", [])
                .unwrap();
            c.execute(
                "INSERT INTO ItemTable VALUES (?1, ?2)",
                [KEY, &json.to_string()],
            )
            .unwrap();
        }
        assert_eq!(
            recent_folders(&db),
            [
                PathBuf::from("d:\\dev\\alpha"),
                PathBuf::from("e:\\beta two")
            ]
        );
        assert!(recent_folders(&d.path().join("missing.vscdb")).is_empty());
    }
}
