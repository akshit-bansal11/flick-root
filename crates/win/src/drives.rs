//! Drive enumeration: which letters exist, what kind they are and which volume sits behind them.
//! Docs: <https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-getlogicaldrives>,
//! <https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-getdrivetypew>,
//! <https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw>.

use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};
use windows::Win32::System::Diagnostics::Debug::{
    SEM_FAILCRITICALERRORS, SetThreadErrorMode, THREAD_ERROR_MODE,
};
use windows::core::PCWSTR;

/// What a drive letter is backed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    /// Local, non-removable disk.
    Fixed,
    /// Removable media such as a USB stick or card.
    Removable,
    /// Network drive.
    Remote,
    /// CD or DVD drive.
    CdRom,
    /// RAM disk.
    RamDisk,
    /// Unknown type or no root directory.
    Unknown,
}

/// One drive worth indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveRoot {
    /// Upper-case drive letter.
    pub letter: char,
    /// Fixed or removable.
    pub kind: DriveKind,
    /// Volume serial number; changes when the volume behind the letter is swapped.
    pub serial: u32,
}

/// Maps a raw `GetDriveTypeW` value (`DRIVE_*` in `fileapi.h`) to a [`DriveKind`].
pub fn kind_from_raw(raw: u32) -> DriveKind {
    match raw {
        2 => DriveKind::Removable,
        3 => DriveKind::Fixed,
        4 => DriveKind::Remote,
        5 => DriveKind::CdRom,
        6 => DriveKind::RamDisk,
        _ => DriveKind::Unknown,
    }
}

/// True for kinds Flick may touch: only fixed and removable disks.
pub fn is_indexable(kind: DriveKind) -> bool {
    matches!(kind, DriveKind::Fixed | DriveKind::Removable)
}

fn root_wide(letter: char) -> Vec<u16> {
    format!("{}:\\", letter.to_ascii_uppercase())
        .encode_utf16()
        .chain([0])
        .collect()
}

/// Bitmask of existing drive letters (bit 0 = `A`). One syscall, never touches a drive.
pub fn logical_drives() -> u32 {
    // SAFETY: GetLogicalDrives takes no arguments and has no preconditions.
    unsafe { GetLogicalDrives() }
}

/// Kind of the drive `letter`. Can block on slow network or optical drives, so call it off the
/// async runtime.
pub fn drive_kind(letter: char) -> DriveKind {
    let wide = root_wide(letter);
    // SAFETY: `wide` is a null-terminated UTF-16 string that outlives the call.
    kind_from_raw(unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) })
}

/// Volume serial of the drive `letter`, or `None` when it has no readable volume (empty card
/// reader, locked or unformatted disk). Critical-error dialogs are suppressed for the call.
pub fn volume_serial(letter: char) -> Option<u32> {
    let wide = root_wide(letter);
    let mut old = THREAD_ERROR_MODE(0);
    // SAFETY: `old` is a valid out pointer for the call.
    let switched = unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, Some(&mut old)) }.is_ok();
    let mut serial = 0u32;
    // SAFETY: `wide` is null-terminated and outlives the call; `serial` is a valid out pointer
    // and every other out argument is None.
    let ok = unsafe {
        GetVolumeInformationW(
            PCWSTR(wide.as_ptr()),
            None,
            Some(&mut serial),
            None,
            None,
            None,
        )
    }
    .is_ok();
    if switched {
        // SAFETY: restores the mode this thread had before; None skips the old-mode out value.
        let _ = unsafe { SetThreadErrorMode(old, None) };
    }
    ok.then_some(serial)
}

/// Drive letters set in a [`logical_drives`] bitmask (bit 0 = `A`). Pure.
pub fn letters(mask: u32) -> Vec<char> {
    ('A'..='Z')
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, c)| c)
        .collect()
}

/// Fixed and removable drives that have a readable volume. Blocking; call off the async runtime.
pub fn indexable_drives() -> Vec<DriveRoot> {
    letters(logical_drives())
        .into_iter()
        .filter_map(|letter| {
            let kind = drive_kind(letter);
            if !is_indexable(kind) {
                return None;
            }
            let serial = volume_serial(letter)?;
            Some(DriveRoot {
                letter,
                kind,
                serial,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_decodes_mask() {
        assert_eq!(letters(0b101), ['A', 'C']);
        assert_eq!(letters(1 << 25), ['Z']);
        assert!(letters(0).is_empty());
    }

    #[test]
    fn raw_kinds_map() {
        assert_eq!(kind_from_raw(0), DriveKind::Unknown);
        assert_eq!(kind_from_raw(1), DriveKind::Unknown);
        assert_eq!(kind_from_raw(2), DriveKind::Removable);
        assert_eq!(kind_from_raw(3), DriveKind::Fixed);
        assert_eq!(kind_from_raw(4), DriveKind::Remote);
        assert_eq!(kind_from_raw(5), DriveKind::CdRom);
        assert_eq!(kind_from_raw(6), DriveKind::RamDisk);
    }

    #[test]
    fn only_fixed_and_removable_are_indexable() {
        let kept: Vec<u32> = (0..8).filter(|r| is_indexable(kind_from_raw(*r))).collect();
        assert_eq!(kept, [2, 3]);
    }

    #[test]
    fn system_drive_is_fixed() {
        let Ok(sd) = std::env::var("SystemDrive") else {
            return;
        };
        let Some(letter) = sd.chars().next() else {
            return;
        };
        assert_eq!(drive_kind(letter), DriveKind::Fixed);
        let up = letter.to_ascii_uppercase();
        assert!(indexable_drives().iter().any(|d| d.letter == up));
    }
}
