# ADR-001: Shell and UI

Status: Accepted (2026-10-19)

## Context

Flick is a keyboard-first launcher for Windows. It needs a window that appears within the hotkey budget, a rich visual design, and all real logic in code that can be tested without a window. The design config sets a high visual bar.

## Decision

- The shell is Tauri 2. All logic lives in Rust crates under `crates/`.
- The UI is React 19 and TypeScript, built with Vite, and rendered by WebView2.
- The launcher window is created at startup, hidden, and only shown and hidden after that. The window is prewarmed, so WebView2 startup cost is paid once.
- v0.0.1 pins Tauri `2.12.4` (`apps/desktop/src-tauri/Cargo.toml`) and React `19.3.1` (`apps/desktop/package.json`). The launcher window starts hidden.
- The Rust package is named `desktop-shell`. The product name, Flick, lives in `tauri.conf.json` and `crates/launcher-core/src/brand.rs`.

## Launcher placement, first revision (2026-10-19, superseded below)

The user instructed this on 2026-10-19, in the v0.0.2/v0.0.3 review. It replaces the earlier decision (700 px wide, 28% from the top of the screen, solid material):

- The bar is centred on the monitor under the cursor, 40% of the monitor width, clamped to 560-960 px.
- The result list is vertically centred when full, and the input stays at a fixed position.
- The window is transparent and covers the whole monitor, with no backdrop. A click outside the bar closes it.
- The window is never resized per keystroke.

## Launcher placement and material (changed 2026-10-19, v0.2.0)

This replaces the full-monitor transparent window above, and with it the user's earlier "remove the bg" request (no backdrop).

- The window is the panel: it is sized to the bar plus its results, and its height follows the results (`resize_launcher`). It is centred on the monitor under the cursor when shown, and re-centred on its monitor at every height change, so the input moves as the list grows (user, 2026-10-22: the fixed input top sat near the top edge of a laptop screen).
- The backdrop is Windows acrylic, with rounded corners, set from `window.rs` and `crates/win/src/dwm.rs`.
- When Windows transparency effects are off, the panel is opaque.

Reason: a transparent full-monitor window has no backdrop to blur, so the "remove the bg" look gave no depth or legibility over busy wallpapers. A panel-sized window can carry an acrylic material, and the OS draws the blur. Whether acrylic stays smooth while the window resizes is not yet checked on a device (see `docs/followups.md`).

## Alternatives

- Pure-Rust UI with egui, iced or Slint. Rejected. Reaching the visual quality of the design config would be much slower.
- Mica instead of acrylic, or a fixed-height window: kept as the fallback if acrylic resizing is not smooth.

## Consequences

- The UI cannot touch the system directly. Commands cross the Tauri IPC boundary, and each window has an allow list in the capability files.
- The WebView makes no network calls. Rust does.
- The content security policy is set in `tauri.conf.json`. v0.0.1 also allows `style-src 'unsafe-inline'`. This is a known deviation that is not yet recorded in an ADR review (see `docs/followups.md`).
- Each launcher show or hide depends on WebView2 staying alive. The cost of a cold WebView2 start is not paid on the hotkey path.
