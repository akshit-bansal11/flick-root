//! Win32 and WinRT wrappers used by several crates: power and session, recycle bin, Explorer restart, radios, media session, port tables and UI Automation. Allowed unsafe code, each block with a SAFETY comment.

pub mod clipboard;
pub mod com;
pub mod drives;
pub mod dwm;
pub mod error;
pub mod explorer;
pub mod foreground;
pub mod media;
pub mod ports;
pub mod power;
pub mod radios;
pub mod recycle;
pub mod thread;
pub mod uia;
pub mod webview;

pub use com::ComGuard;
pub use error::WinError;
