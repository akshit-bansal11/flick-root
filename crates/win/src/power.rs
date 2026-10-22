//! Lock, sleep, hibernate, restart, shutdown and sign-out.
//!
//! Docs relied on:
//! - SetSuspendState: <https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-setsuspendstate>
//!   (needs SE_SHUTDOWN_NAME; "suspend" means S1-S3, so it is never called on an S0 low power idle machine)
//! - SYSTEM_POWER_CAPABILITIES (SystemS3, AoAc = "supports the S0 low power idle model"):
//!   <https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-system_power_capabilities>
//! - Modern Standby: "A Modern Standby session encompasses the entire screen-off to screen-on user
//!   scenario": <https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/modern-standby>
//! - WM_SYSCOMMAND / SC_MONITORPOWER (lParam 2 = display off):
//!   <https://learn.microsoft.com/en-us/windows/win32/menurc/wm-syscommand>
//! - SendMessageTimeoutW (broadcast waits up to the timeout per window):
//!   <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw>
//! - AdjustTokenPrivileges (may succeed with ERROR_NOT_ALL_ASSIGNED and adjust nothing):
//!   <https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-adjusttokenprivileges>

use crate::error::WinError;
use std::time::Duration;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LPARAM, LUID, SetLastError,
    WIN32_ERROR, WPARAM,
};
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
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SC_MONITORPOWER, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SYSCOMMAND,
};
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

/// How this machine goes to sleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepMethod {
    /// Classic S3 sleep through `SetSuspendState`.
    Suspend,
    /// S0 low power idle: turn the display off and Windows enters Modern Standby.
    ModernStandby,
    /// Neither model is reported.
    Unavailable,
}

/// Picks the sleep route from the power capabilities. S0 low power idle wins over S3 because
/// Windows disables S3 when S0 idle is supported, and `SetSuspendState` must never run there.
pub fn sleep_method(s3: bool, aoac: bool) -> SleepMethod {
    if aoac {
        SleepMethod::ModernStandby
    } else if s3 {
        SleepMethod::Suspend
    } else {
        SleepMethod::Unavailable
    }
}

/// Put the machine to sleep: S3 where supported, Modern Standby on S0 low power idle machines.
/// Blocking (waits about 0.5 s on Modern Standby); call from `spawn_blocking`.
pub fn sleep() -> Result<(), WinError> {
    let caps = power_caps()?;
    match sleep_method(caps.SystemS3, caps.AoAc) {
        SleepMethod::Suspend => suspend(false),
        SleepMethod::ModernStandby => modern_standby(),
        SleepMethod::Unavailable => Err(WinError::Unavailable("sleep".into())),
    }
}

fn power_caps() -> Result<SYSTEM_POWER_CAPABILITIES, WinError> {
    let mut caps = SYSTEM_POWER_CAPABILITIES::default();
    // SAFETY: `caps` is a valid, writable SYSTEM_POWER_CAPABILITIES.
    if unsafe { GetPwrCapabilities(&mut caps) } {
        Ok(caps)
    } else {
        Err(WinError::os(
            windows::core::Error::from_thread(),
            "GetPwrCapabilities",
        ))
    }
}

/// Display off through SC_MONITORPOWER, which starts a Modern Standby session.
fn modern_standby() -> Result<(), WinError> {
    // Let the Enter key-up land first, or it wakes the display straight back up.
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: plain FFI call; resets this thread's last-error so a stale code is not reported below.
    unsafe { SetLastError(WIN32_ERROR(0)) };
    // SAFETY: broadcast of a system message with integer params only; SMTO_ABORTIFHUNG plus a
    // 200 ms per-window timeout keeps a hung window from blocking; no out pointer is passed.
    let r = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            WPARAM(SC_MONITORPOWER as usize),
            LPARAM(2),
            SMTO_ABORTIFHUNG,
            200,
            None,
        )
    };
    if r.0 != 0 {
        return Ok(());
    }
    // A zero return on broadcast can mean one window timed out while the display still went off;
    // only a real last-error code is reported.
    let e = windows::core::Error::from_thread();
    if e.code().is_err() {
        Err(WinError::os(e, "SendMessageTimeoutW(SC_MONITORPOWER)"))
    } else {
        Ok(())
    }
}

/// Hibernate (S4). Fails with `Unavailable` when hibernation is off.
pub fn hibernate() -> Result<(), WinError> {
    if !hibernate_available() {
        return Err(WinError::Unavailable("hibernate".into()));
    }
    suspend(true)
}

fn suspend(hibernate: bool) -> Result<(), WinError> {
    enable_shutdown_privilege()?;
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
        .map_err(|e| WinError::os(e, "AdjustTokenPrivileges"))?;
    // SAFETY: no arguments; reads this thread's last-error right after AdjustTokenPrivileges.
    privilege_result(unsafe { GetLastError() })
}

/// AdjustTokenPrivileges can succeed without assigning anything; the last error tells.
fn privilege_result(last: WIN32_ERROR) -> Result<(), WinError> {
    if last == ERROR_NOT_ALL_ASSIGNED {
        Err(WinError::AccessDenied)
    } else {
        Ok(())
    }
}

/// True when the machine can hibernate (hibernation file present and S4 supported). Read-only.
pub fn hibernate_available() -> bool {
    power_caps().is_ok_and(|c| c.HiberFilePresent && c.SystemS4)
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
    fn sleep_method_table() {
        use SleepMethod::*;
        for (s3, aoac, want) in [
            (true, false, Suspend),
            (false, true, ModernStandby),
            (true, true, ModernStandby),
            (false, false, Unavailable),
        ] {
            assert_eq!(sleep_method(s3, aoac), want, "s3={s3} aoac={aoac}");
        }
    }

    #[test]
    fn not_all_assigned_is_access_denied() {
        assert!(matches!(
            privilege_result(ERROR_NOT_ALL_ASSIGNED),
            Err(WinError::AccessDenied)
        ));
        assert!(privilege_result(WIN32_ERROR(0)).is_ok());
    }

    #[test]
    fn hibernate_available_is_read_only() {
        println!("hibernate_available = {}", hibernate_available());
    }
}
