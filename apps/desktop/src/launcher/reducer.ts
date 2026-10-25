import { filterThemes, slotOf } from "../settings/themePicker";
import type { AppearancePatch } from "../settings/types";
import { idFor, type KeyInput } from "../shortcuts";
import type { ThemeView } from "../theme";
import type { AskEvent, Effort, Item, LaunchMode, Mode, Outcome, ThinkingMode } from "../types";
import { isPinnable, THEME_SOURCE, themeRow } from "./kinds";

export type { KeyInput };

/** How long the "Copied" confirmation stays up. */
export const COPIED_MS = 1200;

/** Text captured by Explain this, shown before it is sent. */
export interface Attachment {
  preview: string;
  length: number;
  sourceApp: string;
  blocked: boolean;
  reason: string | null;
}

/** Question used when the user sends an attachment without typing one. */
export const DEFAULT_QUESTION = "Explain this";

export interface QueryState {
  status: "idle" | "streaming" | "done" | "error";
  text: string;
  provider: string | null;
  effort: Effort;
  error: string | null;
  errorCode: string | null;
  thinking: boolean;
  model: string | null;
  /** False once a started event says the provider has no reasoning control. */
  reasoning: boolean;
}

/** The `> theme` quick pick: the list loads once, previews only repaint (no IPC per arrow). */
export interface ThemePickState {
  /** Every theme from list_themes; null until the quick pick of this show asks for it. */
  list: ThemeView[] | null;
  loading: boolean;
  /** Id of the saved theme in use; null when Rust did not resolve one. */
  activeId: string | null;
  /** Id of the theme the launcher is painted with; differs from `activeId` while previewing. */
  painted: string | null;
  /** Id of the selected row last handled, so one selection previews once. */
  seen: string | null;
}

const freshTheme = (activeId: string | null): ThemePickState => ({
  list: null,
  loading: false,
  activeId,
  painted: activeId,
  seen: null,
});

export interface State {
  mode: LaunchMode;
  /** Live source ids from launcher://shown; null until reported, then every prefix is offered. */
  enabledSources: string[] | null;
  /** Title of the first web shortcut (launcher://shown); null when the web feature is off. */
  webTitle: string | null;
  /** True from a search effect until its results arrive; the empty state waits for it. */
  pending: boolean;
  input: string;
  seq: number;
  /** Id of the current ask; events carrying another token are stale and ignored. */
  askToken: number;
  items: Item[];
  /** Recent picks for the blank `all` tab (`home_items`); kept across show and hide. */
  home: Item[];
  /** Ids of pinned items (`home_items`, `toggle_pin`); kept across show and hide. */
  pins: string[];
  selected: number;
  query: QueryState;
  /** Thinking modes (provider + model) Tab cycles through; `selected` is null until chosen. */
  thinkingModes: { modes: ThinkingMode[]; selected: string | null };
  pendingConfirm: { commandId: string; token: string } | null;
  /** True once an arrow key moved the selection; only then does Delete end a process row. */
  armed: boolean;
  /** True while the shortcut sheet covers the body. */
  sheet: boolean;
  /** True for ~1.2 s after a copy action, for the "Copied" confirmation. */
  copied: boolean;
  /** True for the same window after a copy that failed, for "Copy failed". */
  copyFailed: boolean;
  visible: boolean;
  attachment: Attachment | null;
  /** One muted line under the bar, for example "No text selected". */
  notice: string | null;
  theme: ThemePickState;
}

export type Event =
  | { type: "key"; key: KeyInput }
  | { type: "activate"; index: number; ctrl?: boolean; shift?: boolean }
  | { type: "text"; text: string }
  | { type: "results"; seq: number; items: Item[] }
  | { type: "searchFailed"; seq: number }
  | { type: "ask"; token: number; event: AskEvent }
  | { type: "outcome"; commandId: string; outcome: Outcome }
  | { type: "actionFailed"; message: string }
  | { type: "thinkingModes"; modes: ThinkingMode[]; selected: string | null }
  | { type: "tab"; mode: LaunchMode }
  | { type: "homeLoaded"; items: Item[]; pins: string[] }
  | { type: "themesLoaded"; themes: ThemeView[] }
  /** The saved theme in use changed (show, or an apply landed); null when unresolved. */
  | { type: "appearance"; activeId: string | null }
  | { type: "enabledSources"; sources: string[]; webTitle: string | null }
  | { type: "captured"; attachment: Attachment | null }
  | { type: "removeAttachment" }
  | { type: "copyDone"; ok: boolean }
  | { type: "copiedDone" }
  | { type: "shown" }
  | { type: "hidden" };

export type Effect =
  | { type: "search"; seq: number; mode: Mode; text: string; sources: readonly string[] }
  | { type: "launch"; itemId: string; admin: boolean }
  | { type: "confirm"; commandId: string; token: string; kill: boolean }
  | { type: "kill"; itemId: string }
  | { type: "reveal"; itemId: string }
  | { type: "copyPath"; itemId: string }
  | { type: "copyText"; text: string }
  | { type: "clearCopied"; afterMs: number }
  | {
      type: "ask";
      token: number;
      text: string;
      attachment?: string;
      effort: Effort;
      thinkingMode?: string;
    }
  | { type: "cancelAsk"; token: number }
  | { type: "loadThinkingModes" }
  | { type: "loadHome" }
  | { type: "togglePin"; itemId: string }
  | { type: "loadThemes" }
  | { type: "previewTheme"; theme: ThemeView }
  | { type: "revertTheme" }
  | { type: "applyTheme"; patch: AppearancePatch }
  | { type: "webSearch"; text: string }
  | { type: "setThinkingMode"; id: string }
  | { type: "hide" }
  | { type: "openSettings"; section: string };

export interface Result {
  state: State;
  effects: Effect[];
}

/** A mode reachable by a prefix: how it is typed, shown and searched. The one prefix table. */
export interface ModeDef {
  prefix: string;
  label: string;
  placeholder: string;
  /** Hint-panel description. */
  desc: string;
  /** Wire mode sent to Rust `search`; query never searches. */
  wire: Mode;
  /** Source ids sent to `search`; empty for query, which never searches. */
  sources: readonly string[];
  /** Live source ids (launcher://shown); the prefix is offered while one is live. */
  needs: readonly string[];
}

/** Key order is hint order and Ctrl+N order. */
export const MODES: Record<Exclude<LaunchMode, "none">, ModeDef> = {
  find: {
    prefix: "/",
    label: "Open",
    placeholder: "Open an app, file or folder",
    desc: "apps, programs, files and folders",
    wire: "find",
    sources: ["apps", "files", "nlfind"],
    needs: ["apps"],
  },
  commands: {
    prefix: ">",
    label: "Commands",
    placeholder: "Run a command or web shortcut",
    desc: "system, web shortcuts, media, ports and capture",
    wire: "command",
    sources: ["syscmd", "web", "media", "ports", "capture", "links"],
    needs: ["syscmd", "web", "media", "ports", "capture"],
  },
  calc: {
    prefix: "=",
    label: "Calc",
    placeholder: "Math, units, currency or time zones",
    desc: "math, units, currency and time zones",
    wire: "command",
    sources: ["math"],
    needs: ["math"],
  },
  query: {
    prefix: "?",
    label: "Ask",
    placeholder: "Ask anything",
    desc: "your AI providers",
    wire: "query",
    sources: [],
    needs: ["ask"],
  },
  projects: {
    prefix: "@",
    label: "Projects",
    placeholder: "Find a project folder",
    desc: "code folders under your dev folders",
    wire: "command",
    sources: ["projects"],
    needs: ["projects"],
  },
};

type Entered = Exclude<LaunchMode, "none">;
const ENTERED = Object.keys(MODES) as Entered[];
const modeOf = (prefix: string): Entered | undefined =>
  ENTERED.find((m) => MODES[m].prefix === prefix);

/** Modes the user can enter now, in hint-panel order: those with a live source in `needs`. */
export const availableModes = (enabled: string[] | null): Entered[] =>
  ENTERED.filter((m) => enabled === null || MODES[m].needs.some((n) => enabled.includes(n)));

/** Plain text typed with no prefix: apps plus every feature that answers the command wire. */
export const PLAIN_SEARCH = {
  wire: "command" as const,
  sources: ["apps", "features"] as const,
};

export const initialState = (
  enabledSources: string[] | null = null,
  webTitle: string | null = null,
): State => ({
  mode: "none",
  enabledSources,
  webTitle,
  pending: false,
  input: "",
  seq: 0,
  askToken: 0,
  items: [],
  home: [],
  pins: [],
  selected: 0,
  query: {
    status: "idle",
    text: "",
    provider: null,
    effort: "fast",
    error: null,
    errorCode: null,
    thinking: false,
    model: null,
    reasoning: true,
  },
  thinkingModes: { modes: [], selected: null },
  pendingConfirm: null,
  armed: false,
  sheet: false,
  copied: false,
  copyFailed: false,
  visible: false,
  attachment: null,
  notice: null,
  theme: freshTheme(null),
});

const out = (state: State, ...effects: Effect[]): Result => ({ state, effects });

/** The blank `all` tab: none mode, nothing typed. It lists `home` instead of searching. */
const isHome = (s: State): boolean => s.mode === "none" && s.input.trim() === "";

/** Set mode and input; every mode but query searches (none only for non-blank text). */
function search(s: State, mode: LaunchMode, input: string): Result {
  const next: State = {
    ...s,
    mode,
    input,
    selected: 0,
    pendingConfirm: null,
    armed: false,
    pending: false,
  };
  if (mode === "query") return out({ ...next, items: [] });
  if (isHome(next)) {
    const state = { ...next, items: next.home };
    return isHome(s) ? out(state) : out(state, { type: "loadHome" });
  }
  const themeText = themeQuery(next);
  if (themeText !== null) return pickThemes(s, next, themeText);
  const { wire, sources } = mode === "none" ? PLAIN_SEARCH : MODES[mode];
  const seq = s.seq + 1;
  // Plain text shows the fallback rows at once, under the previous results until new ones land.
  const kept = s.mode === "none" && !isHome(s) ? s.items.filter((i) => i.source !== "ui") : [];
  const items = mode === "none" ? [...kept, ...fallbackRows(next)] : next.items;
  return out(
    { ...next, items, seq, pending: true },
    { type: "search", seq, mode: wire, text: input, sources },
  );
}

/** Text after `theme` when the input opens the quick pick (Commands tab), else null. */
function themeQuery(s: State): string | null {
  if (s.mode !== "commands") return null;
  const m = /^theme(?:\s+(.*))?$/i.exec(s.input.trimStart());
  return m ? (m[1] ?? "").trim() : null;
}

/** Rows of the quick pick: themes matching `text`, the saved one tagged current. */
function themeRows(s: State, text: string): Item[] {
  return filterThemes(s.theme.list ?? [], text, "all").map((t) => ({
    id: `theme:${t.id}`,
    kind: "choice",
    title: t.name,
    subtitle: "",
    icon: null,
    score: 0,
    source: THEME_SOURCE,
    actions: [{ type: "primary" }],
    payload: { theme: t, current: t.id === s.theme.activeId },
  }));
}

const currentAt = (items: Item[]): number =>
  Math.max(
    0,
    items.findIndex((i) => themeRow(i)?.current),
  );

/** Shows the quick pick's rows instead of searching; asks for the theme list once. */
function pickThemes(prev: State, next: State, text: string): Result {
  const items = themeRows(next, text);
  // Opening lands on the saved theme; filtering lands on the first match.
  const selected = themeQuery(prev) === null ? currentAt(items) : 0;
  const load = next.theme.list === null && !next.theme.loading;
  const effects: Effect[] = load ? [{ type: "loadThemes" }] : [];
  const theme = load ? { ...next.theme, loading: true } : next.theme;
  // seq moves on so results of earlier keystrokes (t, th...) are dropped.
  return { state: { ...next, theme, items, selected, seq: prev.seq + 1 }, effects };
}

/** Enter or click applies a theme and its scheme; Ctrl+Enter saves its variant slot only. */
function applyThemeRow(s: State, t: ThemeView, withScheme: boolean): Result {
  const scheme = withScheme ? { scheme: t.dark ? ("dark" as const) : ("light" as const) } : {};
  const patch: AppearancePatch =
    slotOf(t) === "darkTheme" ? { darkTheme: t.id, ...scheme } : { lightTheme: t.id, ...scheme };
  return out({ ...s, theme: { ...s.theme, painted: t.id } }, { type: "applyTheme", patch });
}

function switchMode(s: State, mode: LaunchMode, input: string): Result {
  const cancel: Effect[] =
    s.query.status === "streaming" ? [{ type: "cancelAsk", token: s.askToken }] : [];
  const cleared: State = {
    ...s,
    items: [],
    attachment: null,
    notice: null,
    query: {
      ...s.query,
      status: "idle",
      text: "",
      provider: null,
      error: null,
      errorCode: null,
      thinking: false,
      model: null,
    },
  };
  const r = search(cleared, mode, input);
  const load: Effect[] = mode === "query" ? [{ type: "loadThinkingModes" }] : [];
  return { state: r.state, effects: [...cancel, ...r.effects, ...load] };
}

function textChange(s: State, text: string): Result {
  // A prefix counts as the first character into an empty field, typed or pasted.
  const target = s.input === "" ? modeOf(text.slice(0, 1)) : undefined;
  if (target && availableModes(s.enabledSources).includes(target)) {
    return switchMode(s, target, text.slice(1));
  }
  if (s.mode === "query") return out({ ...s, input: text, armed: false, notice: null });
  return search({ ...s, notice: null }, s.mode, text);
}

function move(s: State, delta: number): Result {
  const n = s.items.length;
  if (n === 0) return out(s);
  return out({ ...s, selected: (s.selected + delta + n) % n, armed: true });
}

function enter(s: State): Result {
  if (s.mode === "query") {
    const att = s.attachment;
    // Blocked text is never sent; the Rust ask path refuses it as well.
    if (att?.blocked) return out(s);
    const text = s.input.trim() || (att ? DEFAULT_QUESTION : "");
    if (!text) return out(s);
    const token = s.askToken + 1;
    const cancel: Effect[] =
      s.query.status === "streaming" ? [{ type: "cancelAsk", token: s.askToken }] : [];
    return out(
      {
        ...s,
        askToken: token,
        query: {
          ...s.query,
          status: "streaming",
          text: "",
          provider: null,
          error: null,
          errorCode: null,
          thinking: false,
          model: null,
        },
      },
      ...cancel,
      {
        type: "ask",
        token,
        text,
        attachment: att?.preview,
        effort: s.query.effort,
        thinkingMode: s.thinkingModes.selected ?? undefined,
      },
    );
  }
  return openSelected(s, "open");
}

const FALLBACK_GLYPH: Record<string, string> = { "ui:find": "/", "ui:web": "↗", "ui:ask": "?" };

const uiRow = (id: string, title: string): Item => ({
  id,
  kind: "command",
  title,
  subtitle: "",
  icon: { type: "glyph", value: FALLBACK_GLYPH[id] ?? "" },
  score: 0,
  source: "ui",
  actions: [{ type: "primary" }],
  payload: null,
});

/** C6 rows appended after plain-text results: Find, Web (when a web shortcut exists) and Ask. */
function fallbackRows(s: State): Item[] {
  const text = s.input.trim();
  const sel = s.thinkingModes.modes.find((m) => m.id === s.thinkingModes.selected);
  const ai = sel ? `${sel.providerLabel} · ${sel.model}` : "your AI";
  const rows: Item[] = [];
  if (availableModes(s.enabledSources).includes("find")) {
    rows.push(uiRow("ui:find", `Find files named “${text}”`));
  }
  if (s.webTitle !== null) rows.push(uiRow("ui:web", `Search ${s.webTitle} for “${text}”`));
  if (s.enabledSources === null || s.enabledSources.includes("ask")) {
    rows.push(uiRow("ui:ask", `Ask ${ai}: “${text}”`));
  }
  return rows;
}

/** Enter on a fallback row; Ctrl+Enter and Ctrl+Shift+Enter do nothing there. */
function openFallback(s: State, id: string): Result {
  switch (id) {
    case "ui:find":
      return switchMode(s, "find", s.input);
    case "ui:web":
      return out(s, { type: "webSearch", text: s.input.trim() });
    case "ui:ask": {
      const entered = switchMode(s, "query", s.input);
      const sent = enter(entered.state);
      return { state: sent.state, effects: [...entered.effects, ...sent.effects] };
    }
    default:
      return out(s);
  }
}

type Intent = "open" | "location" | "admin";

/** Enter on the selected row, shared by keyboard and mouse activation. */
function openSelected(s: State, intent: Intent): Result {
  const item = s.items[s.selected];
  if (!item) return out(s);
  const pick = themeRow(item);
  if (pick) return intent === "admin" ? out(s) : applyThemeRow(s, pick.theme, intent === "open");
  if (item.source === "ui") return intent === "open" ? openFallback(s, item.id) : out(s);
  if (intent === "admin") {
    // Rust lists Admin only for apps and exe/msi programs off removable drives.
    if (!item.actions.some((a) => a.type === "admin")) return out(s);
    return out(s, { type: "launch", itemId: item.id, admin: true });
  }
  if (intent === "location") {
    // Rust lists Location only for apps and files; commands, math and links have none.
    if (!item.actions.some((a) => a.type === "location")) return out(s);
    return out(s, { type: "reveal", itemId: item.id });
  }
  const pending = s.pendingConfirm;
  if (pending && pending.commandId === item.id) {
    return out(
      { ...s, pendingConfirm: null },
      {
        type: "confirm",
        commandId: item.id,
        token: pending.token,
        kill: item.kind === "process",
      },
    );
  }
  return out(s, { type: "launch", itemId: item.id, admin: false });
}

function onEscape(s: State): Result {
  if (s.query.status === "streaming") {
    return out(
      { ...s, query: { ...s.query, status: "idle" } },
      { type: "cancelAsk", token: s.askToken },
    );
  }
  // First Esc in the quick pick puts the saved look back; the next one clears the input.
  if (themeQuery(s) !== null && s.theme.painted !== s.theme.activeId) {
    return out({ ...s, theme: { ...s.theme, painted: s.theme.activeId } }, { type: "revertTheme" });
  }
  if (s.input !== "") return textChange(s, "");
  return out(s, { type: "hide" });
}

/** Select the next or previous thinking mode (wraps); persisted through setThinkingMode. */
function cycleThinking(s: State, delta: number): Result {
  const { modes, selected } = s.thinkingModes;
  if (modes.length < 2) return out(s);
  const at = Math.max(
    0,
    modes.findIndex((m) => m.id === selected),
  );
  const next = modes[(at + delta + modes.length) % modes.length];
  if (!next) return out(s);
  return out(
    { ...s, thinkingModes: { modes, selected: next.id } },
    { type: "setThinkingMode", id: next.id },
  );
}

/** The copy itself; "Copied" shows only once the shell reports it landed (`copyDone`). */
const copied = (s: State, copy: Effect): Result => out(s, copy);

/** Text a Commands result copies (`payload.value` when it has a copy action), else null. */
function resultValue(item: Item | undefined): string | null {
  if (!item?.actions.some((a) => a.type === "copyResult")) return null;
  const v = (item.payload as { value?: unknown } | null)?.value;
  return typeof v === "string" ? v : null;
}

/** Select a tab (`none` is all), carrying typed text; a disabled or the current tab does nothing. */
function goMode(s: State, mode: LaunchMode): Result {
  if (mode === s.mode) return out(s);
  if (mode !== "none" && !availableModes(s.enabledSources).includes(mode)) return out(s);
  return switchMode(s, mode, s.input);
}

/** Ctrl+Tab and Ctrl+Shift+Tab: the next or previous tab, `all` first, wrapping. */
function cycleTab(s: State, delta: number): Result {
  const tabs: LaunchMode[] = ["none", ...availableModes(s.enabledSources)];
  const at = Math.max(0, tabs.indexOf(s.mode));
  const next = tabs[(at + delta + tabs.length) % tabs.length];
  return next ? goMode(s, next) : out(s);
}

function key(s: State, k: KeyInput): Result {
  const id = idFor(s.mode, k);
  // Any key the table does not handle (Left, Right, Home, End...) ends "arrowed to a result".
  if (!id) return s.armed ? out({ ...s, armed: false }) : out(s);
  if (s.sheet) {
    // The sheet covers the body: only its toggles and Esc act.
    return id === "sheet" || id === "sheetAlt" || id === "back"
      ? out({ ...s, sheet: false })
      : out(s);
  }
  const item = s.items[s.selected];
  switch (id) {
    case "sheet":
    case "sheetAlt":
      return out({ ...s, sheet: true });
    case "down":
      return move(s, 1);
    case "up":
      return move(s, -1);
    case "open":
    case "ask":
      return enter(s);
    case "location":
      return openSelected(s, "location");
    case "admin":
      return openSelected(s, "admin");
    case "themeSlot":
      return themeRow(item) ? openSelected(s, "location") : out(s);
    case "tabNext":
      return cycleTab(s, 1);
    case "tabPrev":
      return cycleTab(s, -1);
    case "model":
      return cycleThinking(s, 1);
    case "modelPrev":
      return cycleThinking(s, -1);
    case "back":
      return onEscape(s);
    case "kill":
      // Only after an arrow moved the selection; until then Delete edits text.
      return s.armed && item?.kind === "process"
        ? out(s, { type: "kill", itemId: item.id })
        : out(s);
    case "leave":
      // Empty input inside a mode: step back out to the all tab.
      return s.input === "" && s.mode !== "none" ? goMode(s, "none") : out(s);
    case "copyPath":
      return item?.id.startsWith("file:")
        ? copied(s, { type: "copyPath", itemId: item.id })
        : out(s);
    case "copyResult": {
      const value = resultValue(item);
      return value === null ? out(s) : copied(s, { type: "copyText", text: value });
    }
    case "copyAnswer":
      return s.query.text === "" ? out(s) : copied(s, { type: "copyText", text: s.query.text });
    case "removeAttachment":
      return s.attachment ? out({ ...s, attachment: null, notice: null }) : out(s);
    case "effort": {
      const effort: Effort = s.query.effort === "fast" ? "think" : "fast";
      return out({ ...s, query: { ...s.query, effort } });
    }
    case "openSettings":
      return out(s, { type: "openSettings", section: "general" });
    case "pin":
      return item && isPinnable(item) ? out(s, { type: "togglePin", itemId: item.id }) : out(s);
    default: {
      const target = ENTERED.find((m) => id === `go:${m}`);
      return target ? goMode(s, target) : out(s);
    }
  }
}

function ask(s: State, token: number, e: AskEvent): Result {
  const q = s.query;
  if (q.status !== "streaming" || token !== s.askToken) return out(s);
  switch (e.type) {
    case "started":
      return out({
        ...s,
        query: { ...q, provider: e.provider, model: e.model, reasoning: e.reasoning ?? true },
      });
    case "fallback":
      return out({ ...s, query: { ...q, provider: e.to } });
    case "thinking":
      return out({ ...s, query: { ...q, thinking: true } });
    case "delta":
      return out({ ...s, query: { ...q, thinking: false, text: q.text + e.text } });
    case "done":
      return out({ ...s, query: { ...q, status: "done", provider: e.provider } });
    case "error": {
      const errored: State = {
        ...s,
        query: { ...q, status: "error", error: e.message, errorCode: e.code, thinking: false },
      };
      return e.code === "no_provider" || e.code === "query_disabled"
        ? out(errored, { type: "openSettings", section: "providers" })
        : out(errored);
    }
  }
}

function outcome(s: State, commandId: string, o: Outcome): Result {
  return outcomeOf({ ...s, notice: null }, commandId, o);
}

function outcomeOf(s: State, commandId: string, o: Outcome): Result {
  switch (o.type) {
    case "needsConfirm":
      return out({ ...s, pendingConfirm: { commandId, token: o.token } });
    case "showItems":
      return out({ ...s, items: o.items, selected: 0, pendingConfirm: null, armed: false });
    case "hideLauncher":
      return out({ ...s, pendingConfirm: null }, { type: "hide" });
    default:
      return out({ ...s, pendingConfirm: null });
  }
}

/**
 * Keeps the painted theme in step with the quick pick: a newly selected row previews once, and
 * leaving the quick pick with an unsaved look puts the saved theme back.
 */
function settleTheme(r: Result): Result {
  const { state: s } = r;
  const t = s.theme;
  if (themeQuery(s) === null) {
    if (t.painted === t.activeId && t.seen === null) return r;
    const effects: Effect[] =
      t.painted === t.activeId ? r.effects : [...r.effects, { type: "revertTheme" }];
    return { state: { ...s, theme: { ...t, painted: t.activeId, seen: null } }, effects };
  }
  const theme = themeRow(s.items[s.selected])?.theme;
  if (!theme || theme.id === t.seen) return r;
  if (theme.id === t.painted) {
    return { state: { ...s, theme: { ...t, seen: theme.id } }, effects: r.effects };
  }
  return {
    state: { ...s, theme: { ...t, seen: theme.id, painted: theme.id } },
    effects: [...r.effects, { type: "previewTheme", theme }],
  };
}

/** Pure reducer: next state plus effects for the shell to run. */
export function reduce(s: State, ev: Event): Result {
  return settleTheme(step(s, ev));
}

function step(s: State, ev: Event): Result {
  switch (ev.type) {
    case "key":
      return key(s, ev.key);
    case "activate":
      if (!s.items[ev.index]) return out(s);
      return openSelected(
        { ...s, selected: ev.index },
        ev.ctrl && ev.shift ? "admin" : ev.ctrl ? "location" : "open",
      );
    case "text":
      return textChange(s, ev.text);
    case "results":
      if (ev.seq !== s.seq || s.mode === "query" || isHome(s)) return out(s);
      return out({
        ...s,
        items: s.mode === "none" ? [...ev.items, ...fallbackRows(s)] : ev.items,
        selected: 0,
        armed: false,
        pending: false,
      });
    case "searchFailed":
      return ev.seq === s.seq ? out({ ...s, pending: false }) : out(s);
    case "ask":
      return ask(s, ev.token, ev.event);
    case "outcome":
      return outcome(s, ev.commandId, ev.outcome);
    case "actionFailed":
      return out({ ...s, pendingConfirm: null, notice: ev.message });
    case "captured":
      return s.enabledSources === null || s.enabledSources.includes("ask")
        ? out(
            {
              ...s,
              mode: "query",
              input: "",
              items: [],
              selected: 0,
              armed: false,
              pendingConfirm: null,
              attachment: ev.attachment,
              notice: ev.attachment ? null : "No text selected",
            },
            { type: "loadThinkingModes" },
          )
        : out(s);
    case "removeAttachment":
      return out({ ...s, attachment: null, notice: null });
    case "copyDone":
      return ev.ok
        ? out(
            { ...s, copied: true, copyFailed: false },
            { type: "clearCopied", afterMs: COPIED_MS },
          )
        : out(
            { ...s, copied: false, copyFailed: true },
            { type: "clearCopied", afterMs: COPIED_MS },
          );
    case "copiedDone":
      return out({ ...s, copied: false, copyFailed: false });
    case "thinkingModes": {
      const next: State = { ...s, thinkingModes: { modes: ev.modes, selected: ev.selected } };
      // The Ask fallback row names the chosen model; rebuild it when the modes arrive.
      const plainText = next.mode === "none" && !isHome(next);
      const items = plainText
        ? [...next.items.filter((i) => i.source !== "ui"), ...fallbackRows(next)]
        : next.items;
      return out({ ...next, items });
    }
    case "tab":
      return goMode(s, ev.mode);
    case "homeLoaded": {
      const home = ev.items;
      const pins = ev.pins;
      if (!isHome(s)) return out({ ...s, home, pins });
      const selected = Math.min(s.selected, Math.max(0, home.length - 1));
      return out({ ...s, home, pins, items: home, selected });
    }
    case "themesLoaded": {
      const base: State = { ...s, theme: { ...s.theme, list: ev.themes, loading: false } };
      const text = themeQuery(base);
      if (text === null) return out(base);
      const items = themeRows(base, text);
      return out({ ...base, items, selected: currentAt(items) });
    }
    case "appearance": {
      const theme = { ...s.theme, activeId: ev.activeId, painted: ev.activeId };
      const base: State = { ...s, theme };
      const text = themeQuery(base);
      return text === null ? out(base) : out({ ...base, items: themeRows(base, text) });
    }
    case "enabledSources": {
      const load: Effect[] =
        ev.sources.includes("ask") && s.thinkingModes.modes.length === 0
          ? [{ type: "loadThinkingModes" }]
          : [];
      return out({ ...s, enabledSources: ev.sources, webTitle: ev.webTitle }, ...load);
    }
    case "shown":
      return out(
        {
          ...initialState(s.enabledSources, s.webTitle),
          thinkingModes: s.thinkingModes,
          home: s.home,
          pins: s.pins,
          items: s.home,
          seq: s.seq,
          askToken: s.askToken,
          visible: true,
          theme: freshTheme(s.theme.activeId),
        },
        { type: "loadHome" },
      );
    case "hidden": {
      // Hiding mid-preview puts the saved theme back.
      const effects: Effect[] = [
        ...(s.query.status === "streaming"
          ? [{ type: "cancelAsk", token: s.askToken } as const]
          : []),
        ...(s.theme.painted === s.theme.activeId ? [] : [{ type: "revertTheme" } as const]),
      ];
      return {
        state: {
          ...initialState(s.enabledSources, s.webTitle),
          thinkingModes: s.thinkingModes,
          home: s.home,
          pins: s.pins,
          items: s.home,
          seq: s.seq,
          askToken: s.askToken,
          theme: freshTheme(s.theme.activeId),
        },
        effects,
      };
    }
  }
}
