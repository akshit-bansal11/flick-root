//! Raw AppsFolder enumeration via the shell.

use std::path::PathBuf;

use windows::Win32::Storage::EnhancedStorage::PKEY_Link_TargetParsingPath;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItem2, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::{Interface, PWSTR};

use crate::error::AppsError;

/// One unfiltered AppsFolder item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    /// Display name.
    pub name: String,
    /// Parent-relative parsing name; launch with `shell:AppsFolder\<parsing_name>`.
    pub parsing_name: String,
    /// Link target path, absent for packaged apps.
    pub target: Option<PathBuf>,
}

/// Copy a shell-allocated string and free it.
fn take(p: PWSTR) -> String {
    // SAFETY: p came from a successful shell call: valid NUL-terminated, CoTaskMem-allocated; freed once here.
    unsafe {
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn display(item: &IShellItem, kind: SIGDN) -> windows::core::Result<String> {
    // SAFETY: item is a live COM interface; the result string is freed by take.
    unsafe { item.GetDisplayName(kind).map(take) }
}

/// Enumerate every AppsFolder item. The calling thread must hold a `ComGuard`
/// (callers use `spawn_blocking`).
pub fn enumerate_raw() -> Result<Vec<RawEntry>, AppsError> {
    // SAFETY: FOLDERID is a valid static GUID; the out interface type is checked by the generic.
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) }
            .map_err(AppsError::Enumerate)?;
    // SAFETY: folder is live; BHID_EnumItems yields IEnumShellItems.
    let en: IEnumShellItems =
        unsafe { folder.BindToHandler(None, &BHID_EnumItems) }.map_err(AppsError::Enumerate)?;
    let mut out = Vec::new();
    loop {
        let mut slot = [None::<IShellItem>];
        let mut fetched = 0u32;
        // SAFETY: slot and fetched outlive the call.
        unsafe { en.Next(&mut slot, Some(&mut fetched)) }.map_err(AppsError::Enumerate)?;
        let Some(item) = slot[0].take() else {
            break;
        };
        let Ok(name) = display(&item, SIGDN_NORMALDISPLAY) else {
            continue;
        };
        let Ok(parsing_name) = display(&item, SIGDN_PARENTRELATIVEPARSING) else {
            continue;
        };
        let target = item.cast::<IShellItem2>().ok().and_then(|i2| {
            // SAFETY: i2 is live; the key is a static PROPERTYKEY; the string is freed by take.
            unsafe { i2.GetString(&PKEY_Link_TargetParsingPath) }
                .ok()
                .map(take)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        });
        out.push(RawEntry {
            name,
            parsing_name,
            target,
        });
    }
    Ok(out)
}
