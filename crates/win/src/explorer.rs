//! Explorer restart.

use crate::error::WinError;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess,
    WaitForSingleObject,
};

/// Closes the wrapped handle on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful open call and is closed exactly once here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn session_of(pid: u32) -> Option<u32> {
    let mut s = 0u32;
    // SAFETY: `s` is a valid out pointer.
    unsafe { ProcessIdToSessionId(pid, &mut s) }
        .ok()
        .map(|()| s)
}

fn is_explorer(exe: &[u16]) -> bool {
    let len = exe.iter().position(|&c| c == 0).unwrap_or(exe.len());
    String::from_utf16_lossy(&exe[..len]).eq_ignore_ascii_case("explorer.exe")
}

/// Pids of explorer.exe processes in the current session.
fn explorer_pids() -> Result<Vec<u32>, WinError> {
    // SAFETY: no arguments.
    let me = session_of(unsafe { GetCurrentProcessId() });
    // SAFETY: plain FFI call; the snapshot handle is wrapped and closed on drop.
    let snap = Handle(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|e| WinError::os(e, "CreateToolhelp32Snapshot"))?,
    );
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut pids = Vec::new();
    // SAFETY: `snap.0` is a live snapshot; `entry.dwSize` is set; entry is a valid out pointer.
    let mut more = unsafe { Process32FirstW(snap.0, &mut entry) }.is_ok();
    while more {
        if is_explorer(&entry.szExeFile) && session_of(entry.th32ProcessID) == me {
            pids.push(entry.th32ProcessID);
        }
        // SAFETY: same invariants as Process32FirstW above.
        more = unsafe { Process32NextW(snap.0, &mut entry) }.is_ok();
    }
    Ok(pids)
}

/// Terminate every explorer.exe of this session, then start one if Windows did not do so itself.
/// Blocking; call from `spawn_blocking`.
pub fn restart_explorer() -> Result<(), WinError> {
    for pid in explorer_pids()? {
        // SAFETY: plain FFI call; the handle is wrapped and closed on drop.
        let h = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) }
            .map_err(|e| WinError::os(e, "OpenProcess"))?;
        let h = Handle(h);
        // SAFETY: `h.0` is an open process handle with PROCESS_TERMINATE.
        unsafe { TerminateProcess(h.0, 1) }.map_err(|e| WinError::os(e, "TerminateProcess"))?;
        // SAFETY: `h.0` is an open handle with SYNCHRONIZE; bounded wait.
        unsafe { WaitForSingleObject(h.0, 3000) };
    }
    // The shell normally relaunches itself; give it a moment before starting our own.
    if wait_for_explorer(Duration::from_millis(1500))? {
        return Ok(());
    }
    std::process::Command::new("explorer.exe")
        .spawn()
        .map_err(|e| {
            WinError::Failed(format!("Explorer was closed but could not be started: {e}"))
        })?;
    if wait_for_explorer(Duration::from_millis(3000))? {
        Ok(())
    } else {
        Err(WinError::Failed(
            "Explorer was closed and did not come back. Start explorer.exe from Task Manager."
                .into(),
        ))
    }
}

/// True once an explorer.exe of this session is running, polling until `limit` passes.
fn wait_for_explorer(limit: Duration) -> Result<bool, WinError> {
    let deadline = Instant::now() + limit;
    loop {
        if !explorer_pids()?.is_empty() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_match_is_case_insensitive_and_nul_terminated() {
        let mut buf = [0u16; 260];
        for (i, c) in "Explorer.EXE".encode_utf16().enumerate() {
            buf[i] = c;
        }
        assert!(is_explorer(&buf));
        buf[0] = u16::from(b'x');
        assert!(!is_explorer(&buf));
    }
}
