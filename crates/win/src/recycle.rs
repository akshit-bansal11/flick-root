//! Recycle bin.
//!
//! SHEmptyRecycleBinW returns S_OK or an error HRESULT
//! (<https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shemptyrecyclebinw>);
//! in practice an already-empty bin answers E_UNEXPECTED, so that code is only accepted when a
//! fresh query confirms the bin is empty.

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
        Ok(()) => Ok(()),
        Err(e) => {
            let left = if e.code() == E_UNEXPECTED {
                recycle_bin_info().ok().map(|(items, _)| items)
            } else {
                None
            };
            empty_outcome(e, left)
        }
    }
}

/// Maps a failed empty call: success only when the bin is confirmed empty afterwards.
fn empty_outcome(e: windows::core::Error, items_left: Option<u64>) -> Result<(), WinError> {
    match items_left {
        Some(0) => Ok(()),
        Some(n) => Err(WinError::Failed(format!(
            "The recycle bin could not be emptied ({n} items left)."
        ))),
        None => Err(WinError::os(e, "SHEmptyRecycleBinW")),
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
    use super::*;

    #[test]
    fn empty_outcome_maps() {
        let e = || windows::core::Error::from(E_UNEXPECTED);
        assert!(empty_outcome(e(), Some(0)).is_ok());
        assert!(
            matches!(empty_outcome(e(), Some(3)), Err(WinError::Failed(m)) if m.contains("3 items"))
        );
        assert!(matches!(empty_outcome(e(), None), Err(WinError::Os { .. })));
    }

    #[test]
    fn info_is_readable() {
        let (items, bytes) = super::recycle_bin_info().unwrap();
        println!("recycle bin: {items} items, {bytes} bytes");
    }
}
