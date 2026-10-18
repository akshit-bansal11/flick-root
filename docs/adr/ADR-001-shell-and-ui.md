# ADR-001: Shell and UI

Status: Accepted (2026-10-19)

## Context

Flick is a keyboard-first launcher for Windows. It needs a window that appears within the hotkey budget, a rich visual design, and all real logic in code that can be tested without a window. The design config sets a high visual bar.

## Decision

- The shell is Tauri 2. All logic lives in Rust crates under `crates/`.
- The UI is React 19 and TypeScript, built with Vite, and rendered by WebView2.
- The launcher window is created at startup, hidden, and only shown and hidden after that. The window is prewarmed, so WebView2 startup cost is paid once.
- v0.0.1 pins Tauri `2.12.4` (`apps/desktop/src-tauri/Cargo.toml`) and React `19.3.1` (`apps/desktop/package.json`). The launcher window starts hidden at 700 x 64 px.
- The Rust package is named `desktop-shell`. The product name, Flick, lives in `tauri.conf.json` and `crates/launcher-core/src/brand.rs`.

## Alternatives

- Pure-Rust UI with egui, iced or Slint. Rejected. Reaching the visual quality of the design config would be much slower.

## Consequences

- The UI cannot touch the system directly. Commands cross the Tauri IPC boundary, and each window has an allow list in the capability files.
- The WebView makes no network calls. Rust does.
- The content security policy is set in `tauri.conf.json`. v0.0.1 also allows `style-src 'unsafe-inline'`. This is a known deviation that is not yet recorded in an ADR review (see `docs/followups.md`).
- Each launcher show or hide depends on WebView2 staying alive. The cost of a cold WebView2 start is not paid on the hotkey path.
