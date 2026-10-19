//! Apps the AppsFolder misses: per-user installs registered only under the Uninstall keys, and
//! shortcuts that live only on the Desktop.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_PublicDesktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows::core::{GUID, HSTRING, PWSTR};

use crate::enumerate::RawEntry;

const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_WOW: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

// Trailing " 7.01", " v4.31", " 7.36.1", optionally followed by an arch tag.
static VERSION_SUFFIX: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s+(?:v\d+(?:\.\d+)*|\d+(?:\.\d+)+)[a-z]?(?:\s*\((?:x64|x86|64-bit|32-bit)\))?$",
    )
    .ok()
});

/// Display name without a trailing version and arch tag ("WinRAR 7.01 (64-bit)" -> "WinRAR").
pub fn clean_name(name: &str) -> String {
    let n = name.trim();
    match VERSION_SUFFIX.as_ref().and_then(|r| r.find(n)) {
        Some(m) if m.start() > 0 => n[..m.start()].to_string(),
        _ => n.to_string(),
    }
}

/// True for installer, updater and helper executables that are never the app itself.
pub fn is_junk_exe(file_name: &str) -> bool {
    let f = file_name.to_lowercase();
    ["unins", "uninstall", "setup", "install", "elevate"]
        .iter()
        .any(|p| f.starts_with(p))
        || f == "update.exe"
        || f == "squirrel.exe"
        || f.contains("crashpad")
        || f.contains("helper")
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase()
}

/// Reject junk file names and installer-cache locations.
fn acceptable(p: &Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    let junk = p
        .file_name()
        .and_then(|f| f.to_str())
        .is_none_or(is_junk_exe);
    !junk && !s.contains(r"\package cache\") && !s.contains(r"\installer\")
}

/// Executable named by a `DisplayIcon` value, if it is an existing, acceptable `.exe`.
pub fn exe_from_icon(raw: &str, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut s = raw.trim().trim_matches('"').trim();
    if let Some((head, idx)) = s.rsplit_once(',')
        && idx.trim().parse::<i32>().is_ok()
    {
        s = head.trim_end().trim_matches('"');
    }
    let p = PathBuf::from(s.replace('/', "\\"));
    let is_exe = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    (is_exe && acceptable(&p) && exists(&p)).then_some(p)
}

/// Pick the app executable among top-level exe file names for `display_name`.
pub fn pick_exe(candidates: &[String], display_name: &str) -> Option<String> {
    let good: Vec<&String> = candidates.iter().filter(|c| !is_junk_exe(c)).collect();
    let want = norm(display_name);
    let by_name = good.iter().find(|c| {
        let stem = norm(c.rsplit_once('.').map_or(c.as_str(), |(s, _)| s));
        !stem.is_empty() && !want.is_empty() && (stem.contains(&want) || want.contains(&stem))
    });
    match (by_name, good.as_slice()) {
        (Some(c), _) => Some((**c).clone()),
        (None, [only]) => Some((*only).clone()),
        _ => None,
    }
}

/// Owned registry key, closed on drop.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `open` and is closed exactly once here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn open(parent: HKEY, sub: &str) -> Option<Key> {
    let mut h = HKEY::default();
    // SAFETY: `sub` is a valid NUL-terminated HSTRING for the call; `h` is a valid out pointer.
    let r = unsafe { RegOpenKeyExW(parent, &HSTRING::from(sub), None, KEY_READ, &mut h) };
    (r == ERROR_SUCCESS).then_some(Key(h))
}

fn subkeys(k: &Key) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0.. {
        let mut buf = [0u16; 256];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` holds `len` u16s and both outlive the call; other pointers are optional.
        let r = unsafe {
            RegEnumKeyExW(
                k.0,
                i,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if r != ERROR_SUCCESS {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    out
}

enum Value {
    Str(String),
    Dword(u32),
}

fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        match tail
            .find('%')
            .and_then(|b| std::env::var(&tail[..b]).ok().map(|v| (v, b)))
        {
            Some((v, b)) => {
                out.push_str(&v);
                rest = &tail[b + 1..];
            }
            None => {
                out.push('%');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn value(k: &Key, name: &str) -> Option<Value> {
    let name = HSTRING::from(name);
    let mut ty = REG_VALUE_TYPE(0);
    let mut size = 0u32;
    // SAFETY: `name` is NUL-terminated; `ty` and `size` are valid out pointers; no data buffer yet.
    let r = unsafe { RegQueryValueExW(k.0, &name, None, Some(&mut ty), None, Some(&mut size)) };
    if r != ERROR_SUCCESS || size > 65_536 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    // SAFETY: `buf` is `size` bytes and `size` is updated in place; the other pointers are valid.
    let r = unsafe {
        RegQueryValueExW(
            k.0,
            &name,
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    buf.truncate(size as usize);
    match ty {
        t if t == REG_DWORD && buf.len() >= 4 => Some(Value::Dword(u32::from_le_bytes([
            buf[0], buf[1], buf[2], buf[3],
        ]))),
        t if t == REG_SZ || t == REG_EXPAND_SZ => {
            let w: Vec<u16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|&c| c != 0)
                .collect();
            let s = String::from_utf16_lossy(&w);
            Some(Value::Str(if t == REG_EXPAND_SZ {
                expand_env(&s)
            } else {
                s
            }))
        }
        _ => None,
    }
}

fn string(k: &Key, name: &str) -> Option<String> {
    match value(k, name)? {
        Value::Str(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

fn exe_in_dir(dir: &Path, display_name: &str) -> Option<PathBuf> {
    let names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let p = dir.join(pick_exe(&names, display_name)?);
    acceptable(&p).then_some(p)
}

fn uninstall_entries(root: HKEY, path: &str, out: &mut Vec<RawEntry>) {
    let Some(base) = open(root, path) else {
        return;
    };
    for sub in subkeys(&base) {
        let Some(k) = open(base.0, &sub) else {
            continue;
        };
        let Some(display) = string(&k, "DisplayName") else {
            continue;
        };
        if matches!(value(&k, "SystemComponent"), Some(Value::Dword(1)))
            || value(&k, "ParentKeyName").is_some()
        {
            continue;
        }
        let name = clean_name(&display);
        let exe = string(&k, "DisplayIcon")
            .and_then(|i| exe_from_icon(&i, Path::is_file))
            .or_else(|| {
                let loc = string(&k, "InstallLocation")?;
                exe_in_dir(Path::new(loc.trim().trim_matches('"')), &name)
            });
        if let Some(exe) = exe {
            out.push(RawEntry {
                name,
                parsing_name: exe.to_string_lossy().into_owned(),
                target: Some(exe),
            });
        }
    }
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    // SAFETY: `id` is a valid GUID; on success the returned string is NUL-terminated,
    // CoTaskMem-allocated, copied here and freed once.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
    }
}

fn desktop_entries(out: &mut Vec<RawEntry>) {
    for id in [&FOLDERID_Desktop, &FOLDERID_PublicDesktop] {
        let Some(dir) = known_folder(id) else {
            continue;
        };
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            let ok = p.extension().is_some_and(|x| {
                ["lnk", "url", "exe"]
                    .iter()
                    .any(|w| x.eq_ignore_ascii_case(w))
            });
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()).filter(|_| ok) else {
                continue;
            };
            out.push(RawEntry {
                name: stem.to_string(),
                parsing_name: p.to_string_lossy().into_owned(),
                target: Some(p.clone()),
            });
        }
    }
}

/// Extra entries from the Uninstall registry keys and the Desktop folders. Unfiltered.
pub fn extra_entries() -> Vec<RawEntry> {
    let mut out = Vec::new();
    uninstall_entries(HKEY_CURRENT_USER, UNINSTALL, &mut out);
    uninstall_entries(HKEY_LOCAL_MACHINE, UNINSTALL, &mut out);
    uninstall_entries(HKEY_LOCAL_MACHINE, UNINSTALL_WOW, &mut out);
    desktop_entries(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        for (i, o) in [
            ("Notion 7.36.1", "Notion"),
            ("WinRAR 7.01 (64-bit)", "WinRAR"),
            ("WizTree v4.31", "WizTree"),
            ("Open Tools 0.1.0", "Open Tools"),
            ("Spotify", "Spotify"),
            ("Office 365", "Office 365"),
        ] {
            assert_eq!(clean_name(i), o);
        }
    }

    #[test]
    fn icons() {
        let yes = |_: &Path| true;
        for (i, o) in [
            (
                r"C:\Program Files\Google\Chrome\Application\chrome.exe,0",
                Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
            ),
            (
                "\"C:\\Program Files (x86)\\AnyDesk\\AnyDesk.exe\"",
                Some(r"C:\Program Files (x86)\AnyDesk\AnyDesk.exe"),
            ),
            (r"C:\a\b.exe,-12", Some(r"C:\a\b.exe")),
            ("C:/Apps/x/x.exe", Some(r"C:\Apps\x\x.exe")),
            ("C:/Riot Games/Riot Client/Resources/icon.ico", None),
            (r"C:\msys64\uninstall.exe", None),
            (
                r"C:\Users\x\AppData\Local\Package Cache\{guid}\python-3.11.9-amd64.exe,0",
                None,
            ),
        ] {
            assert_eq!(exe_from_icon(i, yes), o.map(PathBuf::from), "{i}");
        }
        assert_eq!(exe_from_icon(r"C:\a\b.exe", |_| false), None);
    }

    #[test]
    fn picks() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            pick_exe(&v(&["Update.exe", "Wispr Flow.exe"]), "Wispr Flow").as_deref(),
            Some("Wispr Flow.exe")
        );
        assert_eq!(
            pick_exe(&v(&["Update.exe", "Wand.exe", "WeMod.exe"]), "Wand").as_deref(),
            Some("Wand.exe")
        );
        assert_eq!(pick_exe(&v(&["a.exe", "b.exe"]), "Zed"), None);
        assert_eq!(
            pick_exe(&v(&["main.exe", "unins000.exe"]), "Zed").as_deref(),
            Some("main.exe")
        );
    }
}
