//! Recycle bin.

use crate::error::WinError;
use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW, SHQUERYRBINFO,
    SHQueryRecycleBinW,
};
use windows::core::PCWSTR;

/// Empty the recycle bin on all drives without prompts. An already-empty bin is success.
pub fn empty_recycle_bin() -> Result<(), WinError> {
    // SAFETY: no window, null root path means all drives; flags are valid SHERB values.
    let r = unsafe {
        SHEmptyRecycleBinW(
            None,
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    };
    match r {
        Err(e) if e.code() != E_UNEXPECTED => Err(WinError::os(e, "SHEmptyRecycleBinW")),
        _ => Ok(()),
    }
}

/// Returns `(items, bytes)` across all drives. Read-only.
pub fn recycle_bin_info() -> Result<(u64, u64), WinError> {
    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: null root path means all drives; `info` has cbSize set and is a valid out pointer.
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }
        .map_err(|e| WinError::os(e, "SHQueryRecycleBinW"))?;
    Ok((info.i64NumItems.max(0) as u64, info.i64Size.max(0) as u64))
}

#[cfg(test)]
mod tests {
    #[test]
    fn info_is_readable() {
        let (items, bytes) = super::recycle_bin_info().unwrap();
        println!("recycle bin: {items} items, {bytes} bytes");
    }
}
