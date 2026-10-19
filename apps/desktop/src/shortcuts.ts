// The one table of keyboard shortcuts. Reducer, footer, action row, sheet, tour and settings read it;
// no chord string lives anywhere else.
import type { LaunchMode } from "./types";

/** A key press as the pure reducer sees it. */
export interface KeyInput {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta?: boolean;
}

export type Scope = LaunchMode | "launcher" | "settings" | "tour";

/** Runtime facts a shortcut can depend on; the sheet explains them, the footer hides the hint without them. */
export type Condition = "adminAction" | "armedProcess" | "copyValue" | "attachment" | "answer";

export const WHEN_TEXT: Record<Condition, string> = {
  adminAction: "result can run as admin",
  armedProcess: "after arrowing to a process",
  copyValue: "result has a value",
  attachment: "text is attached",
  answer: "an answer is shown",
};

export interface Shortcut {
  id: string;
  /** Normalized chord: Ctrl+Alt+Shift+Win order, then the key ("Ctrl+Shift+C", "Esc", "Up"). */
  keys: string;
  label: string;
  /** "launcher" applies in every mode; a mode or "settings" applies there only. */
  scope: Scope | readonly Scope[];
  when?: Condition;
  /** Short footer label; absent means the footer does not show it. */
  foot?: string;
}

const CMD_RUN_FIND = ["commands", "run", "find"] as const;

export const SHORTCUTS: readonly Shortcut[] = [
  {
    id: "kill",
    keys: "Delete",
    label: "End process",
    scope: "commands",
    when: "armedProcess",
    foot: "kill",
  },
  { id: "enterMode", keys: "Enter", label: "Enter mode", scope: "none" },
  { id: "open", keys: "Enter", label: "Open", scope: CMD_RUN_FIND, foot: "open" },
  {
    id: "location",
    keys: "Ctrl+Enter",
    label: "Show location",
    scope: CMD_RUN_FIND,
    foot: "location",
  },
  {
    id: "admin",
    keys: "Ctrl+Shift+Enter",
    label: "Run as admin",
    scope: ["commands", "run"],
    when: "adminAction",
    foot: "admin",
  },
  { id: "copyPath", keys: "Ctrl+Shift+C", label: "Copy path", scope: "find", foot: "copy path" },
  {
    id: "copyResult",
    keys: "Ctrl+Shift+C",
    label: "Copy result",
    scope: "commands",
    when: "copyValue",
    foot: "copy",
  },
  { id: "ask", keys: "Enter", label: "Ask", scope: "query", foot: "ask" },
  {
    id: "copyAnswer",
    keys: "Ctrl+Shift+C",
    label: "Copy answer",
    scope: "query",
    when: "answer",
    foot: "copy answer",
  },
  { id: "model", keys: "Tab", label: "Next model", scope: "query", foot: "model" },
  { id: "modelPrev", keys: "Shift+Tab", label: "Previous model", scope: "query" },
  { id: "modelAlt", keys: "Ctrl+P", label: "Next model", scope: "query" },
  { id: "effort", keys: "Ctrl+T", label: "Fast or Think", scope: "query", foot: "think" },
  {
    id: "removeAttachment",
    keys: "Ctrl+Shift+X",
    label: "Remove attached text",
    scope: "query",
    when: "attachment",
    foot: "remove text",
  },
  { id: "up", keys: "Up", label: "Previous result", scope: "launcher" },
  { id: "down", keys: "Down", label: "Next result", scope: "launcher" },
  { id: "go:commands", keys: "Ctrl+1", label: "Go to Commands", scope: "launcher" },
  { id: "go:run", keys: "Ctrl+2", label: "Go to Apps", scope: "launcher" },
  { id: "go:find", keys: "Ctrl+3", label: "Go to Find", scope: "launcher" },
  { id: "go:query", keys: "Ctrl+4", label: "Go to Ask", scope: "launcher" },
  { id: "leave", keys: "Backspace", label: "Leave mode (empty input)", scope: "launcher" },
  { id: "back", keys: "Esc", label: "Close, clear or hide", scope: "launcher" },
  { id: "openSettings", keys: "Ctrl+,", label: "Open Settings", scope: "launcher" },
  { id: "sheet", keys: "F1", label: "Shortcut list", scope: "launcher", foot: "shortcuts" },
  { id: "sheetAlt", keys: "Ctrl+/", label: "Shortcut list", scope: "launcher" },
  { id: "pageNext", keys: "Ctrl+Tab", label: "Next page", scope: "settings" },
  { id: "pagePrev", keys: "Ctrl+Shift+Tab", label: "Previous page", scope: "settings" },
  { id: "closeSettings", keys: "Esc", label: "Close Settings", scope: "settings" },
  { id: "closeSettingsW", keys: "Ctrl+W", label: "Close Settings", scope: "settings" },
  { id: "tourNext", keys: "Alt+Right", label: "Next tour step", scope: "tour" },
  { id: "tourBack", keys: "Alt+Left", label: "Previous tour step", scope: "tour" },
];

/** Chords text fields and Windows own. No table entry may use one. */
export const RESERVED: readonly string[] = [
  "Ctrl+A",
  "Ctrl+C",
  "Ctrl+V",
  "Ctrl+X",
  "Ctrl+Z",
  "Ctrl+Y",
  "Ctrl+Shift+Z",
  "Ctrl+Backspace",
  "Ctrl+Delete",
  "Ctrl+Left",
  "Ctrl+Right",
  "Ctrl+Shift+Left",
  "Ctrl+Shift+Right",
  "Shift+Left",
  "Shift+Right",
  "Shift+Up",
  "Shift+Down",
  "Shift+Home",
  "Shift+End",
  "Home",
  "End",
  "Ctrl+Home",
  "Ctrl+End",
  "Shift+Delete",
  "Ctrl+Insert",
  "Shift+Insert",
  "Alt+Backspace",
  "Alt+F4",
  "Alt+Tab",
  "Alt+Esc",
  "Ctrl+Esc",
  "F10",
  "Alt",
  "Ctrl+Shift+Esc",
  "Ctrl+Alt+Delete",
];

/** True for a chord the launcher must never claim: listed above or any Win-key chord. */
export const isReserved = (chord: string): boolean =>
  RESERVED.includes(chord) || chord.split("+").includes("Win");

const KEY_NAMES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Esc",
};
const KEY_ORIGINAL = Object.fromEntries(Object.entries(KEY_NAMES).map(([k, v]) => [v, k]));

/** Normalized chord for a key press. */
export function chordOf(k: KeyInput): string {
  const name = KEY_NAMES[k.key] ?? (k.key.length === 1 ? k.key.toUpperCase() : k.key);
  return [k.ctrl && "Ctrl", k.alt && "Alt", k.shift && "Shift", k.meta && "Win", name]
    .filter(Boolean)
    .join("+");
}

/** True when the key press is exactly this chord. */
export const matches = (k: KeyInput, chord: string): boolean => chordOf(k) === chord;

/** Inverse of chordOf, for buttons that send the same key a shortcut would. */
export function parseChord(chord: string): KeyInput {
  const parts = chord.split("+");
  const last = parts.pop() ?? "";
  return {
    key: KEY_ORIGINAL[last] ?? last,
    ctrl: parts.includes("Ctrl"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
    meta: parts.includes("Win"),
  };
}

/** KeyInput from a DOM keydown. */
export const fromEvent = (e: {
  key: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): KeyInput => ({
  key: e.key,
  ctrl: e.ctrlKey,
  shift: e.shiftKey,
  alt: e.altKey,
  meta: e.metaKey,
});

const scopesOf = (s: Shortcut): readonly Scope[] =>
  typeof s.scope === "string" ? [s.scope] : s.scope;

/** Entries declared for exactly this scope. */
export const inScope = (scope: Scope): Shortcut[] =>
  SHORTCUTS.filter((s) => scopesOf(s).includes(scope));

export const shortcut = (id: string): Shortcut => {
  const s = SHORTCUTS.find((x) => x.id === id);
  if (!s) throw new Error(`unknown shortcut ${id}`);
  return s;
};

/** Id of the shortcut a key press triggers in a mode (mode entries first, then launcher-wide). */
export function idFor(mode: LaunchMode | "settings", k: KeyInput): string | undefined {
  const chord = chordOf(k);
  const scopes: Scope[] = mode === "settings" ? ["settings"] : [mode, "launcher"];
  for (const scope of scopes) {
    const hit = inScope(scope).find((s) => s.keys === chord);
    if (hit) return hit.id;
  }
  return undefined;
}

const CAP_NAMES: Record<string, string> = { Enter: "↵", Backspace: "⌫", Up: "↑", Down: "↓" };

/** Key caps of a chord, ready for <kbd>: ["Ctrl", "Shift", "↵"]. */
export const capsOf = (chord: string): string[] => chord.split("+").map((p) => CAP_NAMES[p] ?? p);

/** Caps of a shortcut by id. */
export const caps = (id: string): string[] => capsOf(shortcut(id).keys);

/** Caps as one string, "Ctrl Shift ↵". */
export const capsText = (id: string): string => caps(id).join(" ");

export const SCOPE_LABEL: Record<Scope, string> = {
  launcher: "Anywhere in the launcher",
  none: "Start",
  commands: "Commands",
  run: "Apps",
  find: "Find",
  query: "Ask",
  settings: "Settings window",
  tour: "Tour",
};

const SCOPE_ORDER: Scope[] = [
  "launcher",
  "none",
  "commands",
  "run",
  "find",
  "query",
  "settings",
  "tour",
];

/** The table grouped by scope, for the sheet. */
export const groups = (): { scope: Scope; label: string; items: Shortcut[] }[] =>
  SCOPE_ORDER.map((scope) => ({ scope, label: SCOPE_LABEL[scope], items: inScope(scope) }));
