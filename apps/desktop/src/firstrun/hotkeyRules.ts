// Pure hotkey capture and validation for the first-run step.
export const MSG_NO_MODIFIER = "Add Ctrl, Alt, Shift or Win.";
export const MSG_RESERVED = "Windows keeps this shortcut.";
export const MSG_EXPLAIN = "This shortcut is used for Explain this.";

const MODIFIERS = ["Ctrl", "Alt", "Shift", "Super"];
// Reserved by Windows: chords the OS handles before any app sees them (Win+L locks the PC).
const RESERVED = ["Super+L", "Ctrl+Alt+Delete", "Alt+F4", "Alt+Tab", "Super+D", "Super+Tab"];

interface KeyLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "OS"]);

/** Builds "Ctrl+Alt+Space" from a keydown. Null while only modifiers are held. */
export function comboFromEvent(e: KeyLike): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  let main: string;
  if (e.code === "Space") main = "Space";
  else if (/^Key[A-Z]$/.test(e.code)) main = e.code.slice(3);
  else if (/^Digit\d$/.test(e.code)) main = e.code.slice(5);
  else if (e.key.length === 1) main = e.key.toUpperCase();
  else main = e.key.replace(/^Arrow/, "");
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  return [...mods, main].join("+");
}

const norm = (c: string) => c.toLowerCase().replace(/\bwin\b/g, "super");

/** Returns the user-facing reason a combo is unusable, or null when it may be tried. */
export function validateCombo(combo: string, explainHotkey: string): string | null {
  if (
    !combo
      .split("+")
      .slice(0, -1)
      .some((p) => MODIFIERS.includes(p))
  )
    return MSG_NO_MODIFIER;
  const n = norm(combo);
  if (RESERVED.some((r) => norm(r) === n)) return MSG_RESERVED;
  if (norm(explainHotkey) === n) return MSG_EXPLAIN;
  return null;
}

/** Shows "Super" as "Win" for people. */
export const displayCombo = (c: string) => c.replace(/\bSuper\b/g, "Win");

export const conflictMessage = (combo: string) =>
  `${displayCombo(combo)} is already used by another app. PowerToys Run is a common one. Pick another shortcut.`;
