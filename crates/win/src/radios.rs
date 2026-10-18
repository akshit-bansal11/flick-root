//! Wi-Fi and Bluetooth radios via Windows.Devices.Radios.
//!
//! All functions block on WinRT async operations: call them from `spawn_blocking`.

use crate::com::ComGuard;
use crate::error::WinError;
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind as WinKind, RadioState};

/// Radio families the launcher can toggle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioKind {
    /// Wi-Fi adapter.
    Wifi,
    /// Bluetooth adapter.
    Bluetooth,
}

impl RadioKind {
    fn native(self) -> WinKind {
        match self {
            Self::Wifi => WinKind::WiFi,
            Self::Bluetooth => WinKind::Bluetooth,
        }
    }
}

fn find(kind: RadioKind) -> Result<Option<Radio>, WinError> {
    let access = Radio::RequestAccessAsync()?.join()?;
    if access != RadioAccessStatus::Allowed {
        return Err(WinError::AccessDenied);
    }
    for r in Radio::GetRadiosAsync()?.join()? {
        if r.Kind()? == kind.native() {
            return Ok(Some(r));
        }
    }
    Ok(None)
}

/// `Some(true)` when on, `Some(false)` when off or disabled, `None` when no such radio exists.
pub fn radio_state(kind: RadioKind) -> Result<Option<bool>, WinError> {
    let _com = ComGuard::new_mta()?;
    match find(kind)? {
        Some(r) => Ok(Some(r.State()? == RadioState::On)),
        None => Ok(None),
    }
}

/// Turn a radio on or off. `Unavailable` when the machine has no such radio.
pub fn set_radio(kind: RadioKind, on: bool) -> Result<(), WinError> {
    let _com = ComGuard::new_mta()?;
    let radio = find(kind)?.ok_or_else(|| WinError::Unavailable(format!("{kind:?} radio")))?;
    let state = if on { RadioState::On } else { RadioState::Off };
    if radio.SetStateAsync(state)?.join()? == RadioAccessStatus::Allowed {
        Ok(())
    } else {
        Err(WinError::AccessDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_map() {
        assert_eq!(RadioKind::Wifi.native(), WinKind::WiFi);
        assert_eq!(RadioKind::Bluetooth.native(), WinKind::Bluetooth);
    }

    #[test]
    #[ignore = "live: queries real radios (read-only)"]
    fn radios_read() {
        for k in [RadioKind::Wifi, RadioKind::Bluetooth] {
            println!("{k:?}: {:?}", radio_state(k));
        }
    }
}
