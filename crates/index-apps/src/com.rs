//! COM apartment guard.

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};

/// Initialises COM on the current thread and uninitialises on drop (only if this guard succeeded).
pub struct ComGuard {
    inited: bool,
}

impl ComGuard {
    /// Initialise COM (STA). A thread already in another mode is accepted without uninit.
    pub fn new() -> Result<Self, windows::core::Error> {
        // SAFETY: plain FFI call with no pointers; paired with CoUninitialize in Drop only on success.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if hr.is_ok() {
            Ok(Self { inited: true })
        } else if hr == RPC_E_CHANGED_MODE {
            Ok(Self { inited: false })
        } else {
            Err(hr.into())
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
