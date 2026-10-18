//! RAII COM initialisation.

use crate::error::WinError;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED, CoInitializeEx,
    CoUninitialize,
};

/// Keeps COM initialised on this thread; uninitialises on drop only if this guard initialised it.
pub struct ComGuard {
    inited: bool,
}

impl ComGuard {
    /// Initialise COM as STA. A thread already in another mode is accepted without uninit.
    pub fn new() -> Result<Self, WinError> {
        // SAFETY: plain FFI call with no pointers; paired with CoUninitialize in Drop only on success.
        Self::from_hr(unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
        })
    }

    /// Initialise COM as MTA (for WinRT `.join()` calls). Same tolerance as [`ComGuard::new`].
    ///
    /// The MTA reference is deliberately never released: tearing the MTA down between WinRT
    /// calls on the same thread crashed (access violation) on the next call.
    pub fn new_mta() -> Result<Self, WinError> {
        // SAFETY: plain FFI call with no pointers; the reference is intentionally leaked (see above).
        Self::from_hr(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }).map(|mut g| {
            g.inited = false;
            g
        })
    }

    fn from_hr(hr: windows::core::HRESULT) -> Result<Self, WinError> {
        if hr.is_ok() {
            Ok(Self { inited: true })
        } else if hr == RPC_E_CHANGED_MODE {
            Ok(Self { inited: false })
        } else {
            Err(WinError::os(hr.into(), "CoInitializeEx"))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.inited {
            // SAFETY: balances the successful CoInitializeEx; the guard lives on one thread's stack.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_mismatch_is_tolerated() {
        let g = ComGuard::from_hr(RPC_E_CHANGED_MODE).unwrap();
        assert!(!g.inited);
        let _a = ComGuard::new().unwrap();
        let _b = ComGuard::new_mta().unwrap();
    }
}
