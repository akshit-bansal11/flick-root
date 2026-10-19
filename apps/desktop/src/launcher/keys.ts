import { BROWSER_CHORDS, chordOf, fromEvent, idFor } from "../shortcuts";
import type { LaunchMode } from "../types";

/** Keys that edit text: the default stays unless the reducer actually acted on them. */
const EDITING = new Set(["Backspace", "Delete"]);

/** True for a WebView2 chord (print, reload, zoom, find, navigate...) that must never reach the browser. */
export const isBrowserChord = (e: KeyboardEvent): boolean =>
  BROWSER_CHORDS.includes(chordOf(fromEvent(e)));

/**
 * Whether a launcher keydown must have its default cancelled: Tab, browser chords, and any chord the
 * shortcut table matches, whether or not the reducer changed anything. Backspace and Delete keep their
 * text editing unless the reducer handled them (`handled`).
 */
export function cancelsDefault(e: KeyboardEvent, mode: LaunchMode, handled: boolean): boolean {
  if (e.key === "Tab" || isBrowserChord(e)) return true;
  if (idFor(mode, fromEvent(e)) === undefined) return false;
  return !EDITING.has(e.key) || handled;
}
