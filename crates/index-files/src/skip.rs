//! Skip rules: noisy directories and hidden/system entries are never indexed.

use std::ffi::OsStr;

/// Windows `FILE_ATTRIBUTE_HIDDEN`.
pub const ATTR_HIDDEN: u32 = 0x2;
/// Windows `FILE_ATTRIBUTE_SYSTEM`.
pub const ATTR_SYSTEM: u32 = 0x4;

const SKIP_DIRS: [&str; 18] = [
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    "appdata",
    "$recycle.bin",
    "system volume information",
    "program files",
    "program files (x86)",
    "windows",
    "programdata",
    "recovery",
    "$winreagent",
    "config.msi",
    "onedrivetemp",
    "$sysreset",
];

/// True when an entry must not be indexed (and, for dirs, not descended into).
pub fn should_skip(name: &OsStr, attrs: u32, is_dir: bool) -> bool {
    if attrs & (ATTR_HIDDEN | ATTR_SYSTEM) != 0 {
        return true;
    }
    is_dir
        && name
            .to_str()
            .is_some_and(|n| SKIP_DIRS.iter().any(|s| n.eq_ignore_ascii_case(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        let cases: [(&str, u32, bool, bool); 14] = [
            ("node_modules", 0, true, true),
            ("NODE_MODULES", 0, true, true),
            (".git", 0, true, true),
            ("AppData", 0, true, true),
            ("$Recycle.Bin", 0, true, true),
            ("System Volume Information", 0, true, true),
            ("Program Files (x86)", 0, true, true),
            ("WINDOWS", 0, true, true),
            ("Config.Msi", 0, true, true),
            ("build", 0, false, false),
            ("src", 0, true, false),
            ("a.txt", ATTR_HIDDEN, false, true),
            ("a.txt", ATTR_SYSTEM, false, true),
            ("a.txt", 0x20, false, false),
        ];
        for (n, a, d, want) in cases {
            assert_eq!(should_skip(OsStr::new(n), a, d), want, "{n}");
        }
    }
}
