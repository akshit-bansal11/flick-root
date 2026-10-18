//! Launching apps (normal, runas, reveal location). Never waits on the launched app.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::UI::Shell::{
    SEE_MASK_ASYNCOK, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

use crate::{AppEntry, AppsError};

/// How to launch an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchKind {
    /// Plain `open`.
    Normal,
    /// `runas` (UAC prompt).
    Admin,
}

fn wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// `asynchronous` sets `SEE_MASK_ASYNCOK` (normal launches, 30 ms budget). It must stay off for
/// `runas`: the call could return before the UAC prompt, hiding a cancel.
fn shell_execute(file: &str, verb: &str, name: &str, asynchronous: bool) -> Result<(), AppsError> {
    let file = wide(file);
    let verb = wide(verb);
    let mask = if asynchronous {
        SEE_MASK_FLAG_NO_UI | SEE_MASK_ASYNCOK
    } else {
        SEE_MASK_FLAG_NO_UI
    };
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: mask,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised with cbSize set; `file` and `verb` are NUL-terminated
    // and outlive the call. No process handle is requested, so nothing needs closing.
    let r = unsafe { ShellExecuteExW(&mut info) };
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Err(AppsError::Cancelled),
        Err(source) => Err(AppsError::Launch {
            name: name.to_string(),
            source,
        }),
    }
}

/// Launch an app via `shell:AppsFolder\<parsing_name>`. Needs COM on the calling thread: call in
/// `spawn_blocking` with a `ComGuard`. `Normal` returns without waiting for the app; `Admin` blocks
/// until the UAC prompt is answered, and a cancelled prompt returns `AppsError::Cancelled`.
pub fn launch(entry: &AppEntry, how: LaunchKind) -> Result<(), AppsError> {
    let verb = match how {
        LaunchKind::Normal => "open",
        LaunchKind::Admin => "runas",
    };
    shell_execute(
        &format!("shell:AppsFolder\\{}", entry.parsing_name),
        verb,
        &entry.name,
        how == LaunchKind::Normal,
    )
}

/// Open a file, folder or URL with its default handler. Same threading rules as [`launch`].
pub fn open_path(path: &Path) -> Result<(), AppsError> {
    let p = path.to_string_lossy();
    shell_execute(&p, "open", &p, true)
}

/// Show a path selected in Explorer (spawned, not awaited).
pub fn reveal_path(path: &Path) -> Result<(), AppsError> {
    Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map(drop)
        .map_err(AppsError::Spawn)
}

/// Reveal an app: classic apps select their target in Explorer, packaged apps open `shell:AppsFolder`.
pub fn reveal(entry: &AppEntry) -> Result<(), AppsError> {
    match &entry.target {
        Some(t) => reveal_path(t),
        None => Command::new("explorer.exe")
            .arg("shell:AppsFolder")
            .spawn()
            .map(drop)
            .map_err(AppsError::Spawn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppIndex;
    use std::time::Instant;

    fn kill(image: &str) {
        let _ = Command::new("taskkill").args(["/IM", image, "/F"]).output();
    }

    #[test]
    #[ignore = "launches real apps"]
    fn live_launch_under_30ms() {
        let _com = crate::com::ComGuard::new().unwrap();
        let idx = AppIndex::new();
        idx.refresh_blocking().unwrap();
        for name in ["Calculator", "Notepad"] {
            let Some(e) = idx.entries().iter().find(|a| a.name == name).cloned() else {
                println!("{name}: not present");
                continue;
            };
            let t = Instant::now();
            let r = launch(&e, LaunchKind::Normal);
            let ms = t.elapsed().as_millis();
            println!("launch {name}: {ms} ms, ok={}", r.is_ok());
            r.unwrap();
            assert!(ms < 30, "{name} took {ms} ms");
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        kill("CalculatorApp.exe");
        kill("notepad.exe");
    }
}
