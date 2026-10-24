//! Desktop Window Manager helpers: rounded corners, the caption guard, the dark frame and the system theme setting.

use crate::error::WinError;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongPtrW, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WM_NCDESTROY, WM_STYLECHANGING,
    WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
};
use windows::core::{PCWSTR, w};

/// Asks DWM to round the corners of the window `hwnd` (Windows 11; older systems return an error).
pub fn round_corners(hwnd: isize) -> Result<(), WinError> {
    let pref: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
    // SAFETY: `pref` is a live 4-byte value and the size passed matches it; DWM rejects a stale hwnd with an error.
    unsafe {
        DwmSetWindowAttribute(
            HWND(hwnd as *mut c_void),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const pref).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    }
    .map_err(|e| WinError::os(e, "DwmSetWindowAttribute"))
}

/// `style` without the caption, system menu and min/max box bits.
fn stripped_style(style: u32) -> u32 {
    style & !(WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0)
}

/// Removes the caption buttons and system menu bits from the window style.
pub fn strip_caption(hwnd: isize) -> Result<(), WinError> {
    let h = HWND(hwnd as *mut c_void);
    // SAFETY: plain style get/set on a window handle; a stale hwnd makes the calls fail, not misbehave.
    unsafe {
        let raw = GetWindowLongPtrW(h, GWL_STYLE);
        let style = raw as u32;
        // 0 means the call failed (stale hwnd): writing it back would wipe the style.
        if raw == 0 || stripped_style(style) == style {
            return Ok(());
        }
        SetWindowLongPtrW(h, GWL_STYLE, stripped_style(style) as isize);
        SetWindowPos(
            h,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| WinError::os(e, "SetWindowPos"))
}

/// Fixed id of the caption-guard subclass.
const CAPTION_GUARD_ID: usize = 0x464C_4B31;

unsafe extern "system" fn caption_guard(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_STYLECHANGING && wparam.0 as i32 == GWL_STYLE.0 && lparam.0 != 0 {
        // SAFETY: for WM_STYLECHANGING, lparam points at a live STYLESTRUCT owned by the sender.
        let ss = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
        ss.styleNew = stripped_style(ss.styleNew);
    } else if msg == WM_NCDESTROY {
        // SAFETY: removing our own subclass from the window being destroyed.
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(caption_guard), id) };
    }
    // SAFETY: forwards the unchanged message to the next proc in the subclass chain.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Keeps the caption bits off `hwnd` for good: tao re-applies its styles on show/resize, so a subclass
/// clears them on every `WM_STYLECHANGING`. Must run on the thread that owns the window (Tauri's
/// `setup` is on the main thread; elsewhere use `run_on_main_thread`). Also strips the current style.
pub fn keep_caption_off(hwnd: isize) -> Result<(), WinError> {
    let h = HWND(hwnd as *mut c_void);
    // SAFETY: `caption_guard` is a static proc with the SUBCLASSPROC signature; a repeat install with the same id is a no-op update.
    if !unsafe { SetWindowSubclass(h, Some(caption_guard), CAPTION_GUARD_ID, 0) }.as_bool() {
        return Err(WinError::Unavailable("SetWindowSubclass failed".into()));
    }
    strip_caption(hwnd)
}

/// Whether apps should be dark under the system setting (`AppsUseLightTheme`); missing means dark.
pub fn system_prefers_dark() -> bool {
    personalize_dword(w!("AppsUseLightTheme")).is_none_or(|v| v == 0)
}

/// Reads a REG_DWORD under HKCU `Themes\Personalize`.
fn personalize_dword(name: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: `data` and `len` outlive the call and `len` is the byte size of `data`.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            name,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut data).cast()),
            Some(&raw mut len),
        )
    };
    rc.is_ok().then_some(data)
}

/// Switches the native frame of `hwnd` (caption, border) between dark and light.
pub fn set_dark_frame(hwnd: isize, dark: bool) -> Result<(), WinError> {
    dark_mode(HWND(hwnd as *mut c_void), dark)
}

fn dark_mode(h: HWND, dark: bool) -> Result<(), WinError> {
    let dark = i32::from(dark);
    // SAFETY: `dark` is a live 4-byte BOOL-sized value and the size passed matches it.
    unsafe {
        DwmSetWindowAttribute(
            h,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const dark).cast(),
            size_of::<i32>() as u32,
        )
    }
    .map_err(|e| WinError::os(e, "DwmSetWindowAttribute(dark)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_clears_caption_bits_only() {
        let keep = 0x1000_0000; // WS_VISIBLE
        let all = keep | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
        assert_eq!(stripped_style(all), keep);
        assert_eq!(stripped_style(keep), keep);
    }
}
