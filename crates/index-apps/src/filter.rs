//! Pure filters that drop uninstallers, docs and folders from the raw list.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::enumerate::RawEntry;

// Word boundary: "Help Desk" is dropped, "Helpdesk" is kept.
static JUNK_NAME: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(uninstall|readme|help|documentation|website|release notes|license|manual)\b",
    )
    .ok()
});

// Document-like targets are dropped. Everything else (.exe .msc .cpl .lnk .bat ...) is kept.
const DOC_EXT: &[&str] = &[
    "txt", "pdf", "htm", "html", "chm", "url", "md", "rtf", "doc", "docx", "xps", "log", "ini",
    "xml", "json",
];

/// True unless the display name looks like an uninstaller, readme or similar.
pub fn keep_name(name: &str) -> bool {
    !JUNK_NAME.as_ref().is_some_and(|r| r.is_match(name))
}

/// Keep or drop an entry; `is_dir` answers whether a target path is a directory.
pub fn keep_with(raw: &RawEntry, is_dir: impl Fn(&Path) -> bool) -> bool {
    if JUNK_NAME.as_ref().is_some_and(|r| r.is_match(&raw.name)) {
        return false;
    }
    let Some(t) = &raw.target else {
        return true; // packaged apps have no target
    };
    if let Some(ext) = t.extension().and_then(|e| e.to_str())
        && DOC_EXT.iter().any(|d| ext.eq_ignore_ascii_case(d))
    {
        return false;
    }
    !is_dir(t)
}

/// Keep or drop an entry, checking the disk for directory targets.
pub fn keep(raw: &RawEntry) -> bool {
    keep_with(raw, |p| std::fs::metadata(p).is_ok_and(|m| m.is_dir()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn e(name: &str, target: Option<&str>) -> RawEntry {
        RawEntry {
            name: name.into(),
            parsing_name: name.into(),
            target: target.map(PathBuf::from),
        }
    }

    #[test]
    fn table() {
        let cases = [
            ("Uninstall Foo", Some(r"C:\a\u.exe"), false),
            ("Foo Readme", None, false),
            ("Help", None, false),
            ("Documentation", None, false),
            ("Visit Website", None, false),
            ("License", None, false),
            ("User Manual", None, false),
            ("Release Notes", None, false),
            ("Help Desk", None, false),
            ("Helpdesk", None, true),
            ("Foo", Some(r"C:\Program Files\Foo"), false),
            ("Foo", Some(r"C:\a\guide.PDF"), false),
            ("Spotify", None, true),
            ("Visual Studio Code", Some(r"C:\a\Code.exe"), true),
            ("Services", Some(r"C:\Windows\services.msc"), true),
        ];
        for (n, t, want) in cases {
            let dir = |p: &Path| p.extension().is_none();
            assert_eq!(keep_with(&e(n, t), dir), want, "{n} {t:?}");
        }
    }
}
