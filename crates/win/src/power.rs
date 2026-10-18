//! Lock, sleep, hibernate, restart, shutdown and sign-out.

use crate::error::WinError;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::{
    GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES, SetSuspendState,
};
use windows::Win32::System::Shutdown::{
    EWX_LOGOFF, EWX_POWEROFF, EWX_REBOOT, EXIT_WINDOWS_FLAGS, ExitWindowsEx, LockWorkStation,
    SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER, SHTDN_REASON_MINOR_OTHER, SHUTDOWN_REASON,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PCWSTR;

/// Reason code sent with restart/shutdown/sign-out: "Other, planned" (the user asked for it).
pub const PLANNED_REASON: SHUTDOWN_REASON = SHUTDOWN_REASON(
    SHTDN_REASON_MAJOR_OTHER.0 | SHTDN_REASON_MINOR_OTHER.0 | SHTDN_REASON_FLAG_PLANNED.0,
);

/// Lock the workstation.
pub fn lock() -> Result<(), WinError> {
    // SAFETY: no arguments; acts on the calling session.
    unsafe { LockWorkStation() }.map_err(|e| WinError::os(e, "LockWorkStation"))
}

/// Suspend to sleep (S3).
pub fn sleep() -> Result<(), WinError> {
    suspend(false)
}

/// Hibernate (S4). Fails with `Unavailable` when hibernation is off.
pub fn hibernate() -> Result<(), WinError> {
    if !hibernate_available() {
        return Err(WinError::Unavailable("hibernate".into()));
    }
    suspend(true)
}

fn suspend(hibernate: bool) -> Result<(), WinError> {
    // SAFETY: plain FFI call with three booleans; no pointers.
    if unsafe { SetSuspendState(hibernate, false, false) } {
        Ok(())
    } else {
        Err(WinError::os(
            windows::core::Error::from_thread(),
            "SetSuspendState",
        ))
    }
}

/// Restart the machine.
pub fn restart() -> Result<(), WinError> {
    exit_windows(EWX_REBOOT, true)
}

/// Shut the machine down.
pub fn shutdown() -> Result<(), WinError> {
    exit_windows(EWX_POWEROFF, true)
}

/// Sign the current user out.
pub fn sign_out() -> Result<(), WinError> {
    exit_windows(EWX_LOGOFF, false)
}

fn exit_windows(flags: EXIT_WINDOWS_FLAGS, need_privilege: bool) -> Result<(), WinError> {
    if need_privilege {
        enable_shutdown_privilege()?;
    }
    // SAFETY: plain FFI call with flag values only.
    unsafe { ExitWindowsEx(flags, PLANNED_REASON) }.map_err(|e| WinError::os(e, "ExitWindowsEx"))
}

/// Closes the wrapped handle on drop, covering every exit path.
struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful OpenProcessToken and is closed exactly once here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn enable_shutdown_privilege() -> Result<(), WinError> {
    let mut raw = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo handle needing no close; `raw` is a valid out pointer.
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut raw,
        )
    }
    .map_err(|e| WinError::os(e, "OpenProcessToken"))?;
    let token = Token(raw);
    let mut luid = LUID::default();
    // SAFETY: SE_SHUTDOWN_NAME is a static NUL-terminated wide string; `luid` is a valid out pointer.
    unsafe { LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid) }
        .map_err(|e| WinError::os(e, "LookupPrivilegeValueW"))?;
    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: `token.0` is an open token with ADJUST_PRIVILEGES; `tp` outlives the call; the optional out params are None.
    unsafe { AdjustTokenPrivileges(token.0, false, Some(&tp), 0, None, None) }
        .map_err(|e| WinError::os(e, "AdjustTokenPrivileges"))
}

/// True when the machine can hibernate (hibernation file present and S4 supported). Read-only.
pub fn hibernate_available() -> bool {
    let mut caps = SYSTEM_POWER_CAPABILITIES::default();
    // SAFETY: `caps` is a valid, writable SYSTEM_POWER_CAPABILITIES.
    let ok = unsafe { GetPwrCapabilities(&mut caps) };
    ok && caps.HiberFilePresent && caps.SystemS4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_is_planned_other() {
        assert_eq!(PLANNED_REASON.0, 0x8000_0000);
    }

    #[test]
    fn privilege_name_is_shutdown() {
        // SAFETY: SE_SHUTDOWN_NAME is a valid static wide string.
        assert_eq!(
            unsafe { SE_SHUTDOWN_NAME.to_string() }.unwrap(),
            "SeShutdownPrivilege"
        );
    }

    #[test]
    fn hibernate_available_is_read_only() {
        println!("hibernate_available = {}", hibernate_available());
    }
}
