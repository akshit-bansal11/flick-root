//! Selected text of the focused control: UI Automation first, Ctrl+C with clipboard restore second.
//!
//! These functions initialise COM and sleep; call them from `spawn_blocking`.

use crate::clipboard;
use crate::com::ComGuard;
use crate::error::WinError;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::core::Interface;

/// Maximum characters returned.
const MAX_CHARS: usize = 32 * 1024;
const POLL_MS: u64 = 15;
const POLL_TOTAL_MS: u64 = 300;

fn cap(mut s: String) -> String {
    if let Some((i, _)) = s.char_indices().nth(MAX_CHARS) {
        s.truncate(i);
    }
    s
}

/// Selected text read through the UIA TextPattern of the focused element; `None` if unsupported or empty.
pub fn selected_text_uia() -> Result<Option<String>, WinError> {
    let _com = ComGuard::new()?;
    // SAFETY: COM is initialised by the guard; all interfaces are released on drop.
    unsafe {
        let auto: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| WinError::os(e, "CoCreateInstance(CUIAutomation)"))?;
        let el = auto
            .GetFocusedElement()
            .map_err(|e| WinError::os(e, "GetFocusedElement"))?;
        let Ok(pat) = el.GetCurrentPattern(UIA_TextPatternId) else {
            return Ok(None);
        };
        let Ok(tp) = pat.cast::<IUIAutomationTextPattern>() else {
            return Ok(None);
        };
        let ranges = tp
            .GetSelection()
            .map_err(|e| WinError::os(e, "GetSelection"))?;
        let n = ranges.Length().map_err(|e| WinError::os(e, "Length"))?;
        let mut out = String::new();
        for i in 0..n {
            let r = ranges
                .GetElement(i)
                .map_err(|e| WinError::os(e, "GetElement"))?;
            let t = r.GetText(-1).map_err(|e| WinError::os(e, "GetText"))?;
            out.push_str(&t.to_string());
            if out.chars().count() >= MAX_CHARS {
                break;
            }
        }
        Ok(if out.is_empty() { None } else { Some(cap(out)) })
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Key events for a clean Ctrl+C: release held Alt, Shift and Win, then Ctrl+C.
fn copy_sequence() -> Vec<INPUT> {
    let mut v: Vec<INPUT> = [VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .map(|k| key(k, true))
        .collect();
    v.extend([
        key(VK_CONTROL, false),
        key(VK_C, false),
        key(VK_C, true),
        key(VK_CONTROL, true),
    ]);
    v
}

/// Puts the saved clipboard text back (or empties it) on drop, once our copy changed it.
struct Restore {
    text: Option<String>,
    changed: bool,
}

impl Drop for Restore {
    fn drop(&mut self) {
        if self.changed {
            let _ = clipboard::write_text(self.text.as_deref().unwrap_or(""));
        }
    }
}

/// Selected text of the focused control: UIA, else a simulated Ctrl+C with the clipboard restored.
pub fn selected_text() -> Result<Option<String>, WinError> {
    if let Some(t) = selected_text_uia().ok().flatten() {
        return Ok(Some(t));
    }
    let seq = clipboard::sequence_number();
    let mut guard = Restore {
        text: clipboard::read_text().unwrap_or(None),
        changed: false,
    };
    let inputs = copy_sequence();
    // SAFETY: `inputs` is a valid slice of INPUT and the size argument is size_of::<INPUT>().
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err(WinError::Unavailable("SendInput blocked".into()));
    }
    let mut waited = 0;
    while clipboard::sequence_number() == seq && waited < POLL_TOTAL_MS {
        std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
        waited += POLL_MS;
    }
    if clipboard::sequence_number() == seq {
        return Ok(None);
    }
    guard.changed = true;
    let copied = clipboard::read_text()?;
    Ok(copied.filter(|s| !s.is_empty()).map(cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_sequence_releases_modifiers_then_ctrl_c() {
        let s = copy_sequence();
        assert_eq!(s.len(), 8);
        let flags: Vec<(u16, bool)> = s
            .iter()
            .map(|i| {
                // SAFETY: every INPUT built here is a keyboard event.
                let ki = unsafe { i.Anonymous.ki };
                (ki.wVk.0, ki.dwFlags == KEYEVENTF_KEYUP)
            })
            .collect();
        assert!(flags[..4].iter().all(|f| f.1));
        assert_eq!(flags[4], (VK_CONTROL.0, false));
        assert_eq!(flags[5], (VK_C.0, false));
        assert_eq!(flags[7], (VK_CONTROL.0, true));
    }

    #[test]
    fn cap_truncates_on_char_boundary() {
        let s = "é".repeat(MAX_CHARS + 5);
        assert_eq!(cap(s).chars().count(), MAX_CHARS);
        assert_eq!(cap("abc".into()), "abc");
    }
}
