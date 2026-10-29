# ADR-001: Shell and UI

Status: Accepted (2026-10-19)

## Context

Flick is a keyboard-first launcher for Windows. It needs a window that appears within the hotkey budget, a rich visual design, and all real logic in code that can be tested without a window.

## Decision

- The shell is Tauri 2. All logic lives in Rust crates under `crates/`.
- The UI is React 19 and TypeScript, built with Vite, and rendered by WebView2.
- The launcher window is created at startup, hidden, and only shown and hidden after that. The window is prewarmed, so WebView2 startup cost is paid once.
- Flick pins Tauri `2.12.4` (`apps/desktop/src-tauri/Cargo.toml`) and React `19.3.1` (`apps/desktop/package.json`). The launcher window starts hidden.
- The Rust package is named `desktop-shell`. The product name, Flick, lives in `tauri.conf.json` and `crates/launcher-core/src/brand.rs`.

## Launcher placement and material

- The window is the panel: it is sized once per show to the full panel height and never resizes while open (see ADR-015).
- It is centred on the monitor under the cursor when shown, and re-centred on its monitor at every height change, so the input moves as the list grows. A fixed input position sat too close to the top edge of a laptop screen.
- The backdrop is Windows acrylic with rounded corners, set from `window.rs` and `crates/win/src/dwm.rs`. The palette is the Design Lab dark theme (`#14171D`).
- When Windows transparency effects are off, the panel is opaque.

Reason: a transparent full-monitor window has no backdrop to blur, so it gives no depth or legibility over busy wallpapers. A panel-sized window can carry an acrylic material, and the OS draws the blur. Whether acrylic stays smooth while the window resizes is not yet checked on a device.

## Alternatives

- Pure-Rust UI with egui, iced or Slint. Rejected. Reaching the same visual quality would be much slower.
- Mica instead of acrylic, or a fixed-height window: kept as the fallback if acrylic resizing is not smooth.

## Consequences

- The UI cannot touch the system directly. Commands cross the Tauri IPC boundary, and each window has an allow list in the capability files.
- The WebView makes no network calls. Rust does.
- The content security policy is set in `tauri.conf.json`. It also allows `style-src 'unsafe-inline'`. This is a known deviation.
- Each launcher show or hide depends on WebView2 staying alive. The cost of a cold WebView2 start is not paid on the hotkey path.
