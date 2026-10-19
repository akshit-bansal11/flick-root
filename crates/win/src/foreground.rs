//! The application that owns the foreground window.

use crate::error::WinError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::PWSTR;

/// Closes the process handle on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful OpenProcess and is closed exactly once here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// File name of the foreground window's executable (for example `notepad.exe`); `None` if there is none.
pub fn foreground_app() -> Result<Option<String>, WinError> {
    // SAFETY: GetForegroundWindow takes no input; a null result is handled below.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Ok(None);
    }
    let mut pid = 0u32;
    // SAFETY: `hwnd` is a window handle just returned by the OS and `pid` outlives the call.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    if pid == 0 {
        return Ok(None);
    }
    // SAFETY: plain OpenProcess; the handle is owned by `Handle`, which closes it once.
    let h = Handle(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| WinError::os(e, "OpenProcess"))?,
    );
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` is valid for `len` UTF-16 units and `len` is updated to the number written.
    unsafe {
        QueryFullProcessImageNameW(
            h.0,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
    }
    .map_err(|e| WinError::os(e, "QueryFullProcessImageNameW"))?;
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Ok(path
        .rsplit(std::path::MAIN_SEPARATOR)
        .next()
        .map(str::to_owned))
}
