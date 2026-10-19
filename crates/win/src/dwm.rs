//! Desktop Window Manager helpers: rounded corners, acrylic backdrop and the system theme settings.

use crate::error::WinError;
use std::ffi::c_void;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{PCWSTR, s, w};

/// First Windows 11 build that understands `DWMWA_SYSTEMBACKDROP_TYPE`.
const BACKDROP_MIN_BUILD: u32 = 22621;

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
    from_registry(personalize_dword(w!("EnableTransparency")))
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

fn from_registry(value: Option<u32>) -> bool {
    value != Some(0)
}

/// The Windows build number, or 0 when it cannot be read.
fn build_number() -> u32 {
    let mut buf = [0u16; 16];
    let mut len = size_of_val(&buf) as u32;
    // SAFETY: `buf` and `len` outlive the call and `len` is the byte size of `buf`.
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&raw mut len),
        )
    };
    if rc.is_err() {
        return 0;
    }
    let n = (len as usize / 2).min(buf.len());
    parse_build(&String::from_utf16_lossy(&buf[..n]))
}

fn parse_build(s: &str) -> u32 {
    s.trim_end_matches('\0').trim().parse().unwrap_or(0)
}

/// Whether DWM system backdrops exist on this build.
fn has_backdrop(build: u32) -> bool {
    build >= BACKDROP_MIN_BUILD
}

/// `(frame margin, backdrop type)` for glass on or off. -1 extends the frame over the whole client area.
fn glass_values(on: bool) -> (i32, DWM_SYSTEMBACKDROP_TYPE) {
    if on {
        (-1, DWMSBT_TRANSIENTWINDOW)
    } else {
        (0, DWMSBT_NONE)
    }
}

/// Turns the acrylic backdrop of `hwnd` on or off; `dark` picks the dark tint.
/// Builds before 22621 use the undocumented accent-policy blur instead.
pub fn set_acrylic(hwnd: isize, on: bool, dark: bool) -> Result<(), WinError> {
    let h = HWND(hwnd as *mut c_void);
    if !has_backdrop(build_number()) {
        return accent_blur(h, on);
    }
    let (m, kind) = glass_values(on);
    let margins = MARGINS {
        cxLeftWidth: m,
        cxRightWidth: m,
        cyTopHeight: m,
        cyBottomHeight: m,
    };
    // SAFETY: `margins` is a live MARGINS; DWM rejects a stale hwnd with an error.
    unsafe { DwmExtendFrameIntoClientArea(h, &raw const margins) }
        .map_err(|e| WinError::os(e, "DwmExtendFrameIntoClientArea"))?;
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
    .map_err(|e| WinError::os(e, "DwmSetWindowAttribute(dark)"))?;
    // SAFETY: `kind` is a live 4-byte enum value and the size passed matches it.
    unsafe {
        DwmSetWindowAttribute(
            h,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const kind).cast(),
            size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .map_err(|e| WinError::os(e, "DwmSetWindowAttribute(backdrop)"))
}

/// Mirror of the undocumented `ACCENT_POLICY`.
#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

/// Mirror of the undocumented `WINDOWCOMPOSITIONATTRIBDATA`.
#[repr(C)]
struct CompositionAttrib {
    attrib: u32,
    data: *mut c_void,
    size: usize,
}

type SetWca = unsafe extern "system" fn(HWND, *mut CompositionAttrib) -> i32;

/// Pre-22621 fallback: `SetWindowCompositionAttribute` is not in the SDK, so it is looked up in
/// user32 at runtime. Values: WCA_ACCENT_POLICY = 19, ACCENT_DISABLED = 0, ACCENT_ENABLE_ACRYLICBLURBEHIND = 4.
fn accent_blur(h: HWND, on: bool) -> Result<(), WinError> {
    const WCA_ACCENT_POLICY: u32 = 19;
    // SAFETY: plain lookups in user32, which is loaded in every GUI process.
    let f = unsafe {
        let user32 =
            GetModuleHandleW(w!("user32.dll")).map_err(|e| WinError::os(e, "GetModuleHandleW"))?;
        GetProcAddress(user32, s!("SetWindowCompositionAttribute"))
            .ok_or_else(|| WinError::Unavailable("SetWindowCompositionAttribute".into()))?
    };
    // SAFETY: the export has exactly this signature on every Windows build that has it.
    let f: SetWca = unsafe { std::mem::transmute(f) };
    let mut policy = AccentPolicy {
        state: if on { 4 } else { 0 },
        flags: 0,
        // ABGR; near-zero alpha so the CSS tint decides the colour.
        gradient_color: 0x0100_0000,
        animation_id: 0,
    };
    let mut data = CompositionAttrib {
        attrib: WCA_ACCENT_POLICY,
        data: (&raw mut policy).cast(),
        size: size_of::<AccentPolicy>(),
    };
    // SAFETY: `data` points at `policy`, both live for the call, and `size` matches `policy`.
    if unsafe { f(h, &raw mut data) } == 0 {
        return Err(WinError::Unavailable(
            "SetWindowCompositionAttribute failed".into(),
        ));
    }
    Ok(())
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

    #[test]
    fn build_gate_and_parse() {
        assert!(!has_backdrop(19045));
        assert!(has_backdrop(22621));
        assert!(has_backdrop(26200));
        assert_eq!(parse_build("26200\0"), 26200);
        assert_eq!(parse_build("x"), 0);
    }

    #[test]
    fn glass_values_select() {
        assert_eq!(glass_values(true), (-1, DWMSBT_TRANSIENTWINDOW));
        assert_eq!(glass_values(false), (0, DWMSBT_NONE));
    }
}
