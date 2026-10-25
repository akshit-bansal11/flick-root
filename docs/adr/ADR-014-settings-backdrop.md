# ADR-014: Settings backdrop

Status: Accepted (2026-10-23). Superseded in part by ADR-015 (the glass parts).

## Context

Settings is a normal desktop window with the native title bar. The launcher already sets its acrylic backdrop through DWM (`crates/win/src/dwm.rs`). Mica is the system material meant for long-lived app windows: it tints the window with the wallpaper colour and does not blur live content behind it.

## Decision

- Settings uses Mica (`DWMSBT_MAINWINDOW`) through `win::dwm::set_mica`, the same DWM path as the launcher's acrylic. Tauri's window effects are not used, so the backdrop has one implementation for both windows.
- Mica is on only when the Windows build is 22621 or later (`has_system_backdrop`) and transparency effects are on. Then the window is created with a transparent WebView and the URL carries `?mica=1`. The page sets `data-backdrop="mica"` on `<html>`, keeps the page background clear and paints `--l-mica-tint` (`--l-bg` at 40 %) over the Mica.
- Otherwise (Windows 10, builds before 22621, transparency effects off) the window is opaque and paints `--l-bg`.
- The native title bar stays (`decorations(true)`). Its dark or light mode follows Flick's scheme, not only the system: forced-scheme themes first (frost light; midnight, aurora, ember dark), then `dark` or `light`, then the system for `system`. It is applied when the window opens, after every `set_appearance` and after a system theme change.

## Alternatives

- Custom title bar. Rejected: it loses native snap layouts, the system menu and the caption buttons' accessibility.
- Acrylic in Settings. Rejected: blurred live content behind forms is too busy for a window people read and type in.
- Tauri window effects (`Effect::Mica`). Rejected: a second backdrop path beside `dwm.rs`.

## Consequences

- No new dependency and no new Cargo feature.
- Turning transparency effects on or off while Settings is open takes effect the next time Settings opens.
- The contrast test does not cover Mica: Windows decides its luminance. Text sits on `--l-bg` at 40 % over the OS Mica base, which is itself an opaque, wallpaper-tinted surface.
- CI covers the backdrop values and the scheme table with unit tests. CI cannot prove that Mica shows through the WebView on real hardware, or the caption colour after a theme change. These need a manual pass on a Windows machine before release.
