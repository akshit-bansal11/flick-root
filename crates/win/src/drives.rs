//! Drive type queries.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows::core::PCWSTR;

/// `DRIVE_FIXED` from `fileapi.h`: a local, non-removable disk.
const DRIVE_FIXED: u32 = 3;

/// True when `root` (e.g. `D:\`) is a fixed local disk. Can block on slow network or optical
/// drives, so call it off the async runtime.
pub fn is_fixed_drive(root: &Path) -> bool {
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `wide` is a null-terminated UTF-16 string that outlives the call.
    unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) == DRIVE_FIXED }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_drive_is_fixed() {
        let Ok(sd) = std::env::var("SystemDrive") else {
            return;
        };
        assert!(is_fixed_drive(Path::new(&format!("{sd}\\"))));
    }
}
