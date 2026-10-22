//! Wi-Fi and Bluetooth radios via Windows.Devices.Radios.
//!
//! Docs: RadioAccessStatus <https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radioaccessstatus>,
//! Radio.SetStateAsync <https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radio.setstateasync>.
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
    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
        }
    }

    fn native(self) -> WinKind {
        match self {
            Self::Wifi => WinKind::WiFi,
            Self::Bluetooth => WinKind::Bluetooth,
        }
    }
}

fn find(kind: RadioKind) -> Result<Option<Radio>, WinError> {
    access_result(Radio::RequestAccessAsync()?.join()?, kind)?;
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
    let radio = find(kind)?.ok_or_else(|| WinError::Unavailable(kind.label().into()))?;
    let state = if on { RadioState::On } else { RadioState::Off };
    access_result(radio.SetStateAsync(state)?.join()?, kind)
}

/// Allowed is success; a denial names the radio; Unspecified (or anything new) means unavailable.
fn access_result(status: RadioAccessStatus, kind: RadioKind) -> Result<(), WinError> {
    if status == RadioAccessStatus::Allowed {
        Ok(())
    } else if status == RadioAccessStatus::DeniedByUser
        || status == RadioAccessStatus::DeniedBySystem
    {
        Err(WinError::Denied(kind.label().into()))
    } else {
        Err(WinError::Unavailable(kind.label().into()))
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
    fn access_status_maps() {
        let k = RadioKind::Wifi;
        assert!(access_result(RadioAccessStatus::Allowed, k).is_ok());
        for s in [
            RadioAccessStatus::DeniedByUser,
            RadioAccessStatus::DeniedBySystem,
        ] {
            assert!(matches!(access_result(s, k), Err(WinError::Denied(n)) if n == "Wi-Fi"));
        }
        assert!(matches!(
            access_result(RadioAccessStatus::Unspecified, RadioKind::Bluetooth),
            Err(WinError::Unavailable(n)) if n == "Bluetooth"
        ));
    }

    #[test]
    #[ignore = "live: queries real radios (read-only)"]
    fn radios_read() {
        for k in [RadioKind::Wifi, RadioKind::Bluetooth] {
            println!("{k:?}: {:?}", radio_state(k));
        }
    }
}
