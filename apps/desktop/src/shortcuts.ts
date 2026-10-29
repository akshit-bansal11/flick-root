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
  /** Physical key (KeyboardEvent.code); lets Ctrl+1-4 and Ctrl+/ work on layouts where those need Shift. */
  code?: string;
}

export type Scope = LaunchMode | "launcher" | "settings" | "tour";

/** Runtime facts a shortcut can depend on; the sheet explains them, the footer hides the hint without them. */
export type Condition =
  | "adminAction"
  | "armedProcess"
  | "copyValue"
  | "attachment"
  | "answer"
  | "pathItem"
  | "revealable"
  | "pinnable";

export const WHEN_TEXT: Record<Condition, string> = {
  pinnable: "result is an app, file or folder",
  pathItem: "result is a file or folder",
  revealable: "result has a location",
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

const LIST_SCOPES = ["none", "find", "commands", "calc"] as const;

export const SHORTCUTS: readonly Shortcut[] = [
  {
    id: "kill",
    keys: "Delete",
    label: "End process",
    scope: ["none", "commands"],
    when: "armedProcess",
    foot: "kill",
  },
  { id: "open", keys: "Enter", label: "Open", scope: LIST_SCOPES, foot: "open" },
  {
    id: "location",
    keys: "Ctrl+Enter",
    label: "Show location",
    scope: ["none", "find"],
    when: "revealable",
    foot: "location",
  },
  {
    id: "admin",
    keys: "Ctrl+Shift+Enter",
    label: "Run as admin",
    scope: ["none", "find"],
    when: "adminAction",
    foot: "admin",
  },
  {
    id: "themeSlot",
    keys: "Ctrl+Enter",
    label: "Use theme for its variant only (theme list)",
    scope: "commands",
  },
  {
    id: "copyPath",
    keys: "Ctrl+Shift+C",
    label: "Copy path",
    scope: "find",
    when: "pathItem",
    foot: "copy path",
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
  { id: "model", keys: "Ctrl+T", label: "Next model", scope: "query", foot: "model" },
  { id: "effort", keys: "Shift+Tab", label: "Next effort", scope: "query", foot: "effort" },
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
  { id: "go:find", keys: "Ctrl+1", label: "Go to Open", scope: "launcher" },
  { id: "go:commands", keys: "Ctrl+2", label: "Go to Commands", scope: "launcher" },
  { id: "go:calc", keys: "Ctrl+3", label: "Go to Calc", scope: "launcher" },
  { id: "go:query", keys: "Ctrl+4", label: "Go to Ask", scope: "launcher" },
  { id: "tabNext", keys: "Ctrl+Tab", label: "Next mode tab", scope: "launcher" },
  { id: "tabPrev", keys: "Ctrl+Shift+Tab", label: "Previous mode tab", scope: "launcher" },
  { id: "leave", keys: "Backspace", label: "Leave mode (empty input)", scope: "launcher" },
  { id: "back", keys: "Esc", label: "Close, clear or hide", scope: "launcher" },
  {
    id: "pin",
    keys: "Ctrl+P",
    label: "Pin or unpin",
    scope: ["none", "find"],
    when: "pinnable",
    foot: "pin",
  },
  { id: "openSettings", keys: "Ctrl+,", label: "Open Settings", scope: "launcher" },
  { id: "sheet", keys: "F1", label: "Shortcut list", scope: "launcher", foot: "shortcuts" },
  { id: "sheetAlt", keys: "Ctrl+/", label: "Shortcut list", scope: "launcher" },
  { id: "pageNext", keys: "Ctrl+Tab", label: "Next page", scope: "settings" },
  { id: "pagePrev", keys: "Ctrl+Shift+Tab", label: "Previous page", scope: "settings" },
  { id: "closeSettings", keys: "Esc", label: "Close Settings", scope: "settings" },
  { id: "closeSettingsW", keys: "Ctrl+W", label: "Close Settings", scope: "settings" },
  { id: "tourNext", keys: "PageDown", label: "Next tour step", scope: "tour" },
  { id: "tourBack", keys: "PageUp", label: "Previous tour step", scope: "tour" },
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

/** WebView2 / browser chords: the launcher and tour claim none of them and cancel their default action. */
export const BROWSER_CHORDS: readonly string[] = [
  "Ctrl+F",
  "Ctrl+R",
  "Ctrl+Shift+R",
  "F5",
  "Ctrl+F5",
  "Ctrl+Shift+I",
  "F12",
  "Ctrl++",
  "Ctrl+Shift++",
  "Ctrl+=",
  "Ctrl+-",
  "Ctrl+0",
  "Shift+F10",
  "Alt+Space",
  "Alt+Left",
  "Alt+Right",
  "Ctrl+P",
  "Ctrl+S",
  "Ctrl+N",
  "Ctrl+W",
  "Ctrl+Tab",
  "Ctrl+Shift+Tab",
];

/** Browser chords a scope may claim despite BROWSER_CHORDS (Settings: close, pages; launcher: tabs, pin). */
const OWN_CHORDS: Partial<Record<Scope, readonly string[]>> = {
  settings: ["Ctrl+W", "Ctrl+Tab", "Ctrl+Shift+Tab"],
  launcher: ["Ctrl+Tab", "Ctrl+Shift+Tab"],
  // Pin lives where app and file rows are listed.
  none: ["Ctrl+P"],
  find: ["Ctrl+P"],
};

/** True for a chord a scope must never claim: listed above or any Win-key chord. */
export const isReserved = (chord: string, scope?: Scope): boolean =>
  !(scope !== undefined && (OWN_CHORDS[scope] ?? []).includes(chord)) &&
  (RESERVED.includes(chord) || BROWSER_CHORDS.includes(chord) || chord.split("+").includes("Win"));

const KEY_NAMES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Esc",
};
const KEY_ORIGINAL = Object.fromEntries(Object.entries(KEY_NAMES).map(([k, v]) => [v, k]));

/** Key name; with Ctrl or Alt held, Digit1-4 and Slash come from the physical key (AZERTY needs Shift for digits). */
function nameOf(k: KeyInput): string {
  if (k.code && (k.ctrl || k.alt)) {
    if (/^Digit[1-4]$/.test(k.code)) return k.code.slice(5);
    if (k.code === "Slash") return "/";
  }
  return KEY_NAMES[k.key] ?? (k.key.length === 1 ? k.key.toUpperCase() : k.key);
}

/** Normalized chord for a key press. */
export function chordOf(k: KeyInput): string {
  const name = nameOf(k);
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
  code?: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): KeyInput => ({
  key: e.key,
  code: e.code,
  ctrl: e.ctrlKey,
  shift: e.shiftKey,
  alt: e.altKey,
  meta: e.metaKey,
});

export const scopesOf = (s: Shortcut): readonly Scope[] =>
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

/** A chord as words, lower case: "Ctrl+Shift+Enter" becomes "ctrl+shift+enter". */
export const keyWords = (chord: string): string => chord.toLowerCase();

/**
 * Key names of a chord in their own case: ["Ctrl", "Shift", "Enter"]. They feed
 * aria-keyshortcuts (which wants "Alt", "Control", "Shift" spelled that way); anything shown
 * on screen goes through keyWords.
 */
export const capsOf = (chord: string): string[] => chord.split("+");

/** Caps of a shortcut by id. */
export const caps = (id: string): string[] => capsOf(shortcut(id).keys);

/** A shortcut as words for prose, "ctrl shift enter". */
export const capsText = (id: string): string => keyWords(shortcut(id).keys).split("+").join(" ");

export const SCOPE_LABEL: Record<Scope, string> = {
  launcher: "Anywhere in the launcher",
  none: "Start",
  find: "Open",
  commands: "Commands",
  calc: "Calc",
  query: "Ask",
  settings: "Settings window",
  tour: "Tour",
};

const SCOPE_ORDER: Scope[] = [
  "launcher",
  "none",
  "find",
  "commands",
  "calc",
  "query",
  "settings",
  "tour",
];

/** The table grouped by scope, for the sheet. */
export const groups = (): { scope: Scope; label: string; items: Shortcut[] }[] =>
  SCOPE_ORDER.map((scope) => ({ scope, label: SCOPE_LABEL[scope], items: inScope(scope) }));
