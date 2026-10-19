//! Desktop Window Manager helpers: rounded corners and the system transparency setting.

use crate::error::WinError;
use std::ffi::c_void;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

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

/// Whether the user allows transparency effects (Settings > Personalization > Colors). Missing value means yes.
pub fn transparency_enabled() -> bool {
    let mut data = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: `data` and `len` outlive the call and `len` is the byte size of `data`.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("EnableTransparency"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut data).cast()),
            Some(&raw mut len),
        )
    };
    from_registry(rc.is_ok().then_some(data))
}

fn from_registry(value: Option<u32>) -> bool {
    value != Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_means_enabled() {
        assert!(from_registry(None));
        assert!(from_registry(Some(1)));
        assert!(!from_registry(Some(0)));
    }
}
