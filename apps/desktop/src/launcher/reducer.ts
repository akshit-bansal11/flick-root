import type { AskEvent, Effort, Item, Mode, Outcome } from "../types";

export interface KeyInput {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  /** True when the input has a non-empty text selection (Ctrl+C then copies natively). */
  hasSelection?: boolean;
}

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
  mode: Mode;
  enabledModes: Mode[];
  input: string;
  seq: number;
  /** Id of the current ask; events carrying another token are stale and ignored. */
  askToken: number;
  items: Item[];
  selected: number;
  query: QueryState;
  pendingConfirm: { commandId: string; token: string } | null;
  /** True once an arrow key moved the selection; only then does Backspace kill a process row. */
  armed: boolean;
  overlay: { firstRun: boolean };
  visible: boolean;
}

export type Event =
  | { type: "key"; key: KeyInput }
  | { type: "activate"; index: number; ctrl?: boolean; shift?: boolean }
  | { type: "text"; text: string }
  | { type: "results"; seq: number; items: Item[] }
  | { type: "ask"; token: number; event: AskEvent }
  | { type: "outcome"; commandId: string; outcome: Outcome }
  | { type: "enabledModes"; modes: Mode[] }
  | { type: "shown" }
  | { type: "hidden" };

export type Effect =
  | { type: "search"; seq: number; mode: Mode; text: string }
  | { type: "launch"; itemId: string; admin: boolean }
  | { type: "confirm"; commandId: string; token: string; kill: boolean }
  | { type: "kill"; itemId: string }
  | { type: "reveal"; itemId: string }
  | { type: "copyPath"; itemId: string }
  | { type: "ask"; token: number; text: string; effort: Effort }
  | { type: "cancelAsk"; token: number }
  | { type: "nextProvider" }
  | { type: "hide" }
  | { type: "openSettings"; section: string };

export interface Result {
  state: State;
  effects: Effect[];
}

export const MODE_ORDER: Mode[] = ["command", "find", "query"];
const PREFIX: Record<string, Mode> = { ">": "command", "/": "find", "?": "query" };

export const initialState = (enabledModes: Mode[] = MODE_ORDER): State => ({
  mode: "command",
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
  pendingConfirm: null,
  armed: false,
  overlay: { firstRun: false },
  visible: false,
});

const out = (state: State, ...effects: Effect[]): Result => ({ state, effects });

/** Set mode and input; Command and Find bump seq and search, Query never searches. */
function search(s: State, mode: Mode, input: string): Result {
  const next: State = { ...s, mode, input, selected: 0, pendingConfirm: null, armed: false };
  if (mode === "query") return out({ ...next, items: [] });
  const seq = s.seq + 1;
  return out({ ...next, seq }, { type: "search", seq, mode, text: input });
}

function switchMode(s: State, mode: Mode, input: string): Result {
  const cancel: Effect[] =
    s.query.status === "streaming" ? [{ type: "cancelAsk", token: s.askToken }] : [];
  const cleared: State = {
    ...s,
    items: [],
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
  return { state: r.state, effects: [...cancel, ...r.effects] };
}

function cycle(s: State, dir: 1 | -1): Result {
  const modes = MODE_ORDER.filter((m) => m === "command" || s.enabledModes.includes(m));
  const i = modes.indexOf(s.mode);
  const mode = modes[(i + dir + modes.length) % modes.length] ?? "command";
  return mode === s.mode ? out(s) : switchMode(s, mode, "");
}

function textChange(s: State, text: string): Result {
  // Prefix only counts as the first typed character into an empty field.
  const target = s.input === "" && text.length === 1 ? PREFIX[text] : undefined;
  if (target && (target === "command" || s.enabledModes.includes(target))) {
    return target === s.mode ? search(s, target, "") : switchMode(s, target, "");
  }
  if (s.mode === "query") return out({ ...s, input: text });
  return search(s, s.mode, text);
}

function move(s: State, delta: number): Result {
  const n = s.items.length;
  if (n === 0) return out(s);
  return out({ ...s, selected: (s.selected + delta + n) % n, armed: true });
}

function enter(s: State, k: KeyInput): Result {
  if (s.mode === "query") {
    const text = s.input.trim();
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
      { type: "ask", token, text, effort: s.query.effort },
    );
  }
  return openSelected(s, k);
}

/** Enter on the selected row, shared by keyboard and mouse activation. */
function openSelected(s: State, k: Pick<KeyInput, "ctrl" | "shift">): Result {
  const item = s.items[s.selected];
  if (!item) return out(s);
  if (k.ctrl && k.shift) {
    return s.mode === "command" ? out(s, { type: "launch", itemId: item.id, admin: true }) : out(s);
  }
  if (k.ctrl) return out(s, { type: "reveal", itemId: item.id });
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

function key(s: State, k: KeyInput): Result {
  switch (k.key) {
    case "ArrowDown":
      return move(s, 1);
    case "ArrowUp":
      return move(s, -1);
    case "Enter":
      return enter(s, k);
    case "Tab":
      return cycle(s, k.shift ? -1 : 1);
    case "Escape":
      return onEscape(s);
    case "Backspace": {
      // Edits text until the user arrowed to a row; then it asks to end that process.
      const item = s.items[s.selected];
      return s.armed && item?.kind === "process"
        ? out(s, { type: "kill", itemId: item.id })
        : out(s);
    }
    default:
      break;
  }
  const lower = k.key.toLowerCase();
  if (k.ctrl && !k.shift && lower === "c") {
    const item = s.items[s.selected];
    if (s.mode !== "find" || k.hasSelection || !item) return out(s);
    return out(s, { type: "copyPath", itemId: item.id });
  }
  if (k.ctrl && lower === "t" && s.mode === "query") {
    const effort: Effort = s.query.effort === "fast" ? "think" : "fast";
    return out({ ...s, query: { ...s.query, effort } });
  }
  if (k.ctrl && lower === "p" && s.mode === "query") return out(s, { type: "nextProvider" });
  return out(s);
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
        { ctrl: ev.ctrl ?? false, shift: ev.shift ?? false },
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
    case "enabledModes":
      return out({ ...s, enabledModes: ev.modes });
    case "shown":
      return search(
        { ...initialState(s.enabledModes), seq: s.seq, askToken: s.askToken, visible: true },
        "command",
        "",
      );
    case "hidden":
      return out(
        { ...initialState(s.enabledModes), seq: s.seq, askToken: s.askToken },
        ...(s.query.status === "streaming"
          ? [{ type: "cancelAsk", token: s.askToken } as const]
          : []),
      );
  }
}
