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
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};
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

/// Outcome of a UIA probe: no TextPattern, TextPattern with nothing selected, or the text.
enum Uia {
    Unsupported,
    Empty,
    Text(String),
}

/// Selected text read through the UIA TextPattern of the focused element; `None` if unsupported or empty.
pub fn selected_text_uia() -> Result<Option<String>, WinError> {
    Ok(match probe()? {
        Uia::Text(t) => Some(t),
        _ => None,
    })
}

fn probe() -> Result<Uia, WinError> {
    let _com = ComGuard::new()?;
    // SAFETY: COM is initialised by the guard; all interfaces are released on drop.
    unsafe {
        let auto: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| WinError::os(e, "CoCreateInstance(CUIAutomation)"))?;
        let el = auto
            .GetFocusedElement()
            .map_err(|e| WinError::os(e, "GetFocusedElement"))?;
        let Ok(pat) = el.GetCurrentPattern(UIA_TextPatternId) else {
            return Ok(Uia::Unsupported);
        };
        let Ok(tp) = pat.cast::<IUIAutomationTextPattern>() else {
            return Ok(Uia::Unsupported);
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
        Ok(if out.is_empty() {
            Uia::Empty
        } else {
            Uia::Text(cap(out))
        })
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

/// Window classes where Ctrl+C interrupts the running process instead of copying.
fn is_terminal_class(class: &str) -> bool {
    matches!(
        class,
        "ConsoleWindowClass" | "CASCADIA_HOSTING_WINDOW_CLASS" | "VirtualConsoleClass" | "mintty"
    )
}

/// Executables that host a terminal (standalone or embedded) where Ctrl+C can mean SIGINT.
fn is_terminal_host_app(exe: &str) -> bool {
    const HOSTS: &[&str] = &[
        "code.exe",
        "code - insiders.exe",
        "cursor.exe",
        "windsurf.exe",
        "zed.exe",
        "windowsterminal.exe",
        "conhost.exe",
        "openconsole.exe",
        "wezterm-gui.exe",
        "alacritty.exe",
        "mintty.exe",
        "putty.exe",
        "hyper.exe",
        "tabby.exe",
        "idea64.exe",
        "pycharm64.exe",
        "webstorm64.exe",
        "rider64.exe",
        "clion64.exe",
        "goland64.exe",
        "rustrover64.exe",
        "studio64.exe",
    ];
    HOSTS.contains(&exe.to_lowercase().as_str())
}

/// Class name of the foreground window; empty if there is none.
fn foreground_class() -> String {
    let mut buf = [0u16; 256];
    // SAFETY: GetForegroundWindow takes no input; `buf` is a valid writable slice for GetClassNameW,
    // which returns the number of characters written (0 on failure).
    let n = unsafe {
        let hwnd = GetForegroundWindow();
        GetClassNameW(hwnd, &mut buf)
    };
    String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)])
}

/// Selected text of the focused control: UIA, else a simulated Ctrl+C with the clipboard restored.
pub fn selected_text() -> Result<Option<String>, WinError> {
    match probe() {
        Ok(Uia::Text(t)) => return Ok(Some(t)),
        // Supported but nothing selected: a Ctrl+C would only copy nothing (or interrupt a process).
        Ok(Uia::Empty) => return Ok(None),
        _ => {}
    }
    if is_terminal_class(&foreground_class()) {
        return Ok(None);
    }
    // Apps with an embedded terminal share one window class with their editor, so go by exe:
    // Ctrl+C there may interrupt a process, and an unknown app is safer than a lost build.
    let app = crate::foreground::foreground_app().ok().flatten();
    if app.as_deref().is_none_or(is_terminal_host_app) {
        return Ok(None);
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
    fn terminal_classes_never_get_ctrl_c() {
        assert!(is_terminal_class("ConsoleWindowClass"));
        assert!(is_terminal_class("CASCADIA_HOSTING_WINDOW_CLASS"));
        assert!(!is_terminal_class("Notepad"));
        assert!(!is_terminal_class(""));
        assert!(is_terminal_host_app("Code.exe"));
        assert!(is_terminal_host_app("WindowsTerminal.exe"));
        assert!(!is_terminal_host_app("notepad.exe"));
    }

    #[test]
    fn cap_truncates_on_char_boundary() {
        let s = "é".repeat(MAX_CHARS + 5);
        assert_eq!(cap(s).chars().count(), MAX_CHARS);
        assert_eq!(cap("abc".into()), "abc");
    }
}
