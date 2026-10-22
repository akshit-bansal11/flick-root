//! Error type for this crate.

use windows::Win32::Foundation::E_ACCESSDENIED;

/// Failure of a Windows API call.
#[derive(thiserror::Error, Debug)]
pub enum WinError {
    /// An API failed with this HRESULT or Win32 code.
    #[error("{context} failed with code {code:#x}")]
    Os {
        /// The raw HRESULT (or Win32 error) value.
        code: i32,
        /// Which operation failed.
        context: &'static str,
    },
    /// The OS or the user refused the request.
    #[error("access denied")]
    AccessDenied,
    /// The capability does not exist on this system.
    #[error("not available on this system: {0}")]
    Unavailable(String),
    /// Windows refused the request for this named thing (for example "Wi-Fi").
    #[error("Windows denied access to {0}")]
    Denied(String),
    /// The call returned but the job was not done; the text says what is wrong.
    #[error("{0}")]
    Failed(String),
    /// The operation was cancelled.
    #[error("cancelled")]
    Cancelled,
}

impl WinError {
    /// Wrap an OS error with the name of the failing operation.
    pub fn os(e: windows::core::Error, context: &'static str) -> Self {
        if e.code() == E_ACCESSDENIED {
            Self::AccessDenied
        } else {
            Self::Os {
                code: e.code().0,
                context,
            }
        }
    }
}

impl From<windows::core::Error> for WinError {
    fn from(e: windows::core::Error) -> Self {
        Self::os(e, "windows call")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_preserved_and_access_denied_mapped() {
        let e = WinError::os(
            windows::core::Error::from_hresult(windows::core::HRESULT(-2147467259)),
            "x",
        );
        assert!(matches!(
            e,
            WinError::Os {
                code: -2147467259,
                ..
            }
        ));
        assert!(matches!(
            WinError::from(windows::core::Error::from(E_ACCESSDENIED)),
            WinError::AccessDenied
        ));
    }
}
