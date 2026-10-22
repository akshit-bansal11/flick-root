# ADR-013: Tray menu

Status: Accepted (2026-10-23)

## Context

The tray icon opened a native Win32 menu. It cannot be themed, has no glass, and looks unlike the launcher it belongs to. Flick already keeps one WebView alive and hidden (the prewarmed launcher window) so the launcher opens at once.

## Decision

- The tray menu is a surface of the prewarmed launcher WebView. A right-click on the tray icon places and sizes that window, then emits `tray://menu` with the app version, the launcher hotkey, any pending update, and the glass and appearance settings.
- The window root renders the menu on `tray://menu` and goes back to the launcher on `launcher://hidden` and on `launcher://shown`. While the menu shows, the launcher is not mounted, so its keyboard handling cannot act on menu keys. The root acknowledges a show that arrives in that state, because the launcher cannot.
- The menu is a `role="menu"` list with a roving selection lens. Open, Settings, Check for updates (or Install, once one is found) and Quit run through the `tray_action` command.
- `tray_action` is accepted from the launcher window only. Every action except Quit hides the window first. Escape and focus loss hide the window.
- Size is fixed: `MENU_W` and `MENU_H` in `window.rs` equal `--l-menu-w` and `--l-menu-h` in `tokens.css`. Position comes from `menu_layout`, a pure function of the work area, the scale and the click point. A click left or right of the work area puts the menu beside it. Otherwise the menu opens above a click in the lower half and below a click in the upper half. The result is clamped inside the work area.

## Alternatives

- Keep the native menu. Rejected: it cannot be styled and cannot show glass.
- A second, dedicated WebView for the menu. Rejected: another WebView process costs idle memory, and a cold WebView is slower to show than the one that is already warm.
- Render the menu inside the Settings window. Rejected: Settings is a normal window that is usually closed, so the menu would pay a window creation on every right-click.

## Consequences

- No new process and no new dependency. The menu opens as fast as the launcher does.
- The launcher window serves two surfaces, so a stale `tray://menu` or a missed `launcher://hidden` would leave the wrong surface up. The root clears the menu on both hide and show.
- CI covers `menu_layout` with unit tests, the menu reducer and component, and the root swapping surfaces on the events. CI cannot prove:
  - placement on real multi-monitor setups with mixed scale, and with a vertical or auto-hidden taskbar;
  - that focus moves into the menu after a right-click and that clicking elsewhere hides it, which depends on how Windows hands focus to a window shown from the tray;
  - the acrylic backdrop under the menu on real hardware.
  These need a manual pass on a Windows machine before release.
