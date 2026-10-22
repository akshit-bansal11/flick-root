//! WebView2 memory management while the launcher is hidden: low memory target plus suspend.
//! See learn.microsoft.com/microsoft-edge/webview2/concepts/performance ("Use memory management APIs").

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    ICoreWebView2_3, ICoreWebView2_19, ICoreWebView2Controller,
};
use webview2_com::TrySuspendCompletedHandler;
use windows::core::Interface;

use crate::error::WinError;

/// Call after the webview is hidden: sets the memory target to low, then asks the runtime to
/// suspend (TrySuspend only works while the webview is invisible). An old runtime without
/// these interfaces is a silent no-op.
pub fn on_hidden(c: &ICoreWebView2Controller) -> Result<(), WinError> {
    // TrySuspend fails unless the controller is invisible; hiding the HWND does not set this.
    // SAFETY: `c` is a live COM interface owned by the caller; the call takes a plain bool.
    unsafe { c.SetIsVisible(false) }?;
    // SAFETY: `c` is a live COM interface owned by the caller; CoreWebView2 only reads it.
    let core = unsafe { c.CoreWebView2() }?;
    if let Ok(w) = core.cast::<ICoreWebView2_19>() {
        // SAFETY: `w` is a valid interface pointer; the call takes a plain enum value.
        unsafe { w.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW) }?;
    }
    if let Ok(w) = core.cast::<ICoreWebView2_3>() {
        let done = TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(())));
        // SAFETY: `w` and the handler are valid COM objects; the runtime keeps its own reference.
        unsafe { w.TrySuspend(&done) }?;
    }
    Ok(())
}

/// Call before the webview is shown: resumes it if suspended, then restores the normal
/// memory target. An old runtime without these interfaces is a silent no-op.
pub fn on_shown(c: &ICoreWebView2Controller) -> Result<(), WinError> {
    // Microsoft's sample order: Resume, then IsVisible(TRUE). Visibility is restored even when
    // resuming fails, so a failed resume can never leave the shown launcher blank (the WebView
    // also auto-resumes when it becomes visible).
    let resumed = resume_normal(c);
    // SAFETY: `c` is a live COM interface owned by the caller; the call takes a plain bool.
    unsafe { c.SetIsVisible(true) }?;
    resumed
}

fn resume_normal(c: &ICoreWebView2Controller) -> Result<(), WinError> {
    // SAFETY: `c` is a live COM interface owned by the caller; CoreWebView2 only reads it.
    let core = unsafe { c.CoreWebView2() }?;
    if let Ok(w) = core.cast::<ICoreWebView2_3>() {
        let mut suspended = windows::core::BOOL(0);
        // SAFETY: `suspended` outlives the call and is a valid out pointer.
        unsafe { w.IsSuspended(&mut suspended) }?;
        if suspended.as_bool() {
            // SAFETY: `w` is a valid interface pointer.
            unsafe { w.Resume() }?;
        }
    }
    if let Ok(w) = core.cast::<ICoreWebView2_19>() {
        // SAFETY: `w` is a valid interface pointer; the call takes a plain enum value.
        unsafe { w.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL) }?;
    }
    Ok(())
}
