import { idFor, type KeyInput } from "../shortcuts";
import type {
  AskEvent,
  Effort,
  Item,
  LaunchMode,
  Mode,
  Outcome,
  ThinkingMode,
} from "../types";
import { isProgram } from "./kinds";

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

export interface State {
  mode: LaunchMode;
  enabledModes: Mode[];
  input: string;
  seq: number;
  /** Id of the current ask; events carrying another token are stale and ignored. */
  askToken: number;
  items: Item[];
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
}

export type Event =
  | { type: "key"; key: KeyInput }
  | { type: "activate"; index: number; ctrl?: boolean; shift?: boolean }
  | { type: "text"; text: string }
  | { type: "results"; seq: number; items: Item[] }
  | { type: "ask"; token: number; event: AskEvent }
  | { type: "outcome"; commandId: string; outcome: Outcome }
  | { type: "actionFailed"; message: string }
  | { type: "thinkingModes"; modes: ThinkingMode[]; selected: string | null }
  | { type: "enterMode"; mode: LaunchMode }
  | { type: "enabledModes"; modes: Mode[] }
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
    sources: ["syscmd", "web", "media", "ports", "capture"],
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

/** Modes the user can enter now, in hint-panel order; Query and Find follow enabledModes. */
export const availableModes = (enabled: Mode[]): Entered[] =>
  ENTERED.filter((m) => MODES[m].wire === "command" || enabled.includes(MODES[m].wire));

export const initialState = (enabledModes: Mode[] = ["command", "find", "query"]): State => ({
  mode: "none",
  enabledModes,
  input: "",
  seq: 0,
  askToken: 0,
  items: [],
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
});

const out = (state: State, ...effects: Effect[]): Result => ({ state, effects });

/** Set mode and input; every mode but none and query bumps seq and searches its sources. */
function search(s: State, mode: LaunchMode, input: string): Result {
  const next: State = { ...s, mode, input, selected: 0, pendingConfirm: null, armed: false };
  if (mode === "none" || mode === "query") return out({ ...next, items: [] });
  const { wire, sources } = MODES[mode];
  const seq = s.seq + 1;
  return out({ ...next, seq }, { type: "search", seq, mode: wire, text: input, sources });
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

/** Enter a mode; plain text typed on the hint panel is carried into it. */
function enterMode(s: State, mode: LaunchMode): Result {
  if (mode !== "none" && !availableModes(s.enabledModes).includes(mode)) return out(s);
  return switchMode(s, mode, mode !== "none" && s.mode === "none" ? s.input : "");
}

function textChange(s: State, text: string): Result {
  // A prefix counts as the first character into an empty field, typed or pasted.
  const target = s.input === "" ? modeOf(text.slice(0, 1)) : undefined;
  if (target && availableModes(s.enabledModes).includes(target)) {
    return switchMode(s, target, text.slice(1));
  }
  if (s.mode === "none" || s.mode === "query") {
    return out({ ...s, input: text, armed: false, notice: null });
  }
  return search({ ...s, notice: null }, s.mode, text);
}

function move(s: State, delta: number): Result {
  const n = s.mode === "none" ? availableModes(s.enabledModes).length : s.items.length;
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
  if (s.mode === "none") {
    const mode = availableModes(s.enabledModes)[s.selected];
    return mode ? enterMode(s, mode) : out(s);
  }
  return openSelected(s, "open");
}

type Intent = "open" | "location" | "admin";

/** Enter on the selected row, shared by keyboard and mouse activation. */
function openSelected(s: State, intent: Intent): Result {
  const item = s.items[s.selected];
  if (!item) return out(s);
  if (intent === "admin") {
    const admin = s.mode === "commands" || s.mode === "find";
    // Scanned programs only offer Admin when Rust listed it (.exe/.msi).
    if (!admin || (isProgram(item) && !item.actions.some((a) => a.type === "admin"))) return out(s);
    return out(s, { type: "launch", itemId: item.id, admin: true });
  }
  if (intent === "location") return out(s, { type: "reveal", itemId: item.id });
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

/** Ctrl+1..5: jump to a mode, carrying typed text; disabled modes and the current mode do nothing. */
function goMode(s: State, mode: Entered): Result {
  if (mode === s.mode || !availableModes(s.enabledModes).includes(mode)) return out(s);
  return switchMode(s, mode, s.input);
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
    case "enterMode":
    case "open":
    case "ask":
      return enter(s);
    case "location":
      return openSelected(s, "location");
    case "admin":
      return openSelected(s, "admin");
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
      // Empty input inside a mode: step back out to the hint panel.
      return s.input === "" && s.mode !== "none" ? enterMode(s, "none") : out(s);
    case "copyPath":
      return item ? copied(s, { type: "copyPath", itemId: item.id }) : out(s);
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

/** Pure reducer: next state plus effects for the shell to run. */
export function reduce(s: State, ev: Event): Result {
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
      if (ev.seq !== s.seq) return out(s);
      return out({ ...s, items: ev.items, selected: 0, armed: false });
    case "ask":
      return ask(s, ev.token, ev.event);
    case "outcome":
      return outcome(s, ev.commandId, ev.outcome);
    case "actionFailed":
      return out({ ...s, pendingConfirm: null, notice: ev.message });
    case "captured":
      return s.enabledModes.includes("query")
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
    case "thinkingModes":
      return out({ ...s, thinkingModes: { modes: ev.modes, selected: ev.selected } });
    case "enterMode":
      return enterMode(s, ev.mode);
    case "enabledModes":
      return out({ ...s, enabledModes: ev.modes });
    case "shown":
      return out({
        ...initialState(s.enabledModes),
        thinkingModes: s.thinkingModes,
        seq: s.seq,
        askToken: s.askToken,
        visible: true,
      });
    case "hidden":
      return out(
        {
          ...initialState(s.enabledModes),
          thinkingModes: s.thinkingModes,
          seq: s.seq,
          askToken: s.askToken,
        },
        ...(s.query.status === "streaming"
          ? [{ type: "cancelAsk", token: s.askToken } as const]
          : []),
      );
  }
}
