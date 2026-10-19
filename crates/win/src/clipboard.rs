//! Clipboard text access (CF_UNICODETEXT).
//!
//! ponytail: only text is saved and restored; images, files and other formats are not preserved.
//! Upgrade path: enumerate formats with EnumClipboardFormats and copy each handle.

use crate::error::WinError;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::core::PCWSTR;

/// Clipboard format id of UTF-16 text (avoids the Win32_System_Ole feature).
const CF_UNICODETEXT: u32 = 13;

const OPEN_TRIES: u32 = 8;

/// Counter that changes whenever the clipboard content changes.
pub fn sequence_number() -> u32 {
    // SAFETY: no arguments, no pointers.
    unsafe { GetClipboardSequenceNumber() }
}

struct Open;

impl Open {
    fn new() -> Result<Self, WinError> {
        let mut last = None;
        for _ in 0..OPEN_TRIES {
            // SAFETY: null owner window is allowed; paired with CloseClipboard in Drop on success.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Self),
                Err(e) => last = Some(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        Err(last.map_or(WinError::Cancelled, |e| WinError::os(e, "OpenClipboard")))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by this guard on this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Current clipboard text, or `None` if it holds no text.
pub fn read_text() -> Result<Option<String>, WinError> {
    let _open = Open::new()?;
    // SAFETY: clipboard is open; the returned handle is owned by the clipboard and only read.
    let h = match unsafe { GetClipboardData(CF_UNICODETEXT) } {
        Ok(h) if !h.is_invalid() => h,
        _ => return Ok(None),
    };
    let g = HGLOBAL(h.0);
    // SAFETY: g is a movable global block from the clipboard; locked, read only within its
    // GlobalSize (a missing NUL cannot run past the block), unlocked.
    unsafe {
        let p = GlobalLock(g) as *const u16;
        if p.is_null() {
            return Ok(None);
        }
        let max = GlobalSize(g) / 2;
        let all = std::slice::from_raw_parts(p, max);
        let len = all.iter().position(|&c| c == 0).unwrap_or(max);
        let s = String::from_utf16_lossy(&all[..len]);
        let _ = GlobalUnlock(g);
        Ok(Some(s))
    }
}

/// Puts `data` on the open clipboard under the registered format `name`.
///
/// # Safety
/// The clipboard must be open on this thread and already emptied by the caller.
unsafe fn set_registered(name: &str, data: &[u8]) {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated and outlives the call. The block is sized for `data`,
    // written while locked, and freed unless SetClipboardData takes ownership.
    unsafe {
        let fmt = RegisterClipboardFormatW(PCWSTR(wide.as_ptr()));
        if fmt == 0 {
            return;
        }
        let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1)) else {
            return;
        };
        let p = GlobalLock(g) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(g));
            return;
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
        let _ = GlobalUnlock(g);
        if SetClipboardData(fmt, Some(HANDLE(g.0))).is_err() {
            let _ = GlobalFree(Some(g));
        }
    }
}

/// Replace the clipboard content with `text`, marked so clipboard history, cloud clipboard and
/// clipboard monitors skip it (the app only writes back restores; they must not leave new copies).
pub fn write_text(text: &str) -> Result<(), WinError> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = wide.len() * 2;
    let _open = Open::new()?;
    // SAFETY: clipboard is open. The block is sized for `wide`, written while locked, and freed
    // here unless SetClipboardData succeeds (then the clipboard owns it).
    unsafe {
        EmptyClipboard().map_err(|e| WinError::os(e, "EmptyClipboard"))?;
        let g = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| WinError::os(e, "GlobalAlloc"))?;
        let p = GlobalLock(g) as *mut u16;
        if p.is_null() {
            let _ = GlobalFree(Some(g));
            return Err(WinError::Unavailable("GlobalLock".into()));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
        let _ = GlobalUnlock(g);
        if let Err(e) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(g.0))) {
            let _ = GlobalFree(Some(g));
            return Err(WinError::os(e, "SetClipboardData"));
        }
        set_registered("ExcludeClipboardContentFromMonitorProcessing", &[0]);
        set_registered("CanIncludeInClipboardHistory", &0u32.to_le_bytes());
        set_registered("CanUploadToCloudClipboard", &0u32.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "touches the real clipboard"]
    fn round_trip_restores() {
        let before = read_text().unwrap();
        write_text("launcher-test").unwrap();
        assert_eq!(read_text().unwrap().as_deref(), Some("launcher-test"));
        write_text(before.as_deref().unwrap_or("")).unwrap();
        assert_eq!(
            read_text().unwrap().unwrap_or_default(),
            before.unwrap_or_default()
        );
    }
}
