//! Background processing mode for the current thread (low CPU and I/O priority).

use crate::error::WinError;
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_MODE_BACKGROUND_END,
};

/// Puts the calling thread in background mode until [`leave_background_mode`] or thread exit.
pub fn enter_background_mode() -> Result<(), WinError> {
    // SAFETY: GetCurrentThread returns a pseudo-handle valid for this thread; no pointers are passed.
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) }
        .map_err(|e| WinError::os(e, "SetThreadPriority(background begin)"))
}

/// Ends background mode on the calling thread.
pub fn leave_background_mode() -> Result<(), WinError> {
    // SAFETY: as in `enter_background_mode`.
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END) }
        .map_err(|e| WinError::os(e, "SetThreadPriority(background end)"))
}

/// Background mode for the guard's lifetime; pooled threads are reused, so drop restores normal priority.
pub struct BackgroundGuard {
    entered: bool,
}

impl BackgroundGuard {
    /// Enters background mode; a failure is tolerated and the guard then does nothing on drop.
    pub fn new() -> Self {
        Self {
            entered: enter_background_mode().is_ok(),
        }
    }
}

impl Default for BackgroundGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BackgroundGuard {
    fn drop(&mut self) {
        if self.entered {
            let _ = leave_background_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_enters_and_leaves() {
        let g = BackgroundGuard::new();
        drop(g);
        // A second cycle works only if the first one left background mode.
        assert!(enter_background_mode().is_ok());
        assert!(leave_background_mode().is_ok());
    }
}
