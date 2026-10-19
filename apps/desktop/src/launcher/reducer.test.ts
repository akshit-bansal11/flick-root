import { describe, expect, it } from "vitest";
import type { Item, LaunchMode } from "../types";
import {
  type Attachment,
  type Event,
  initialState,
  type KeyInput,
  MODES,
  reduce,
  type State,
} from "./reducer";

const item = (id: string): Item => ({
  id,
  kind: "app",
  title: id,
  subtitle: "",
  icon: null,
  score: 1,
  source: "apps",
  actions: [{ type: "primary" }],
  payload: null,
});
const k = (key: string, o: Partial<KeyInput> = {}): Event => ({
  type: "key",
  key: { key, ctrl: false, shift: false, alt: false, ...o },
});
const withItems = (s: State, n = 3): State => ({
  ...s,
  items: Array.from({ length: n }, (_, i) => item(`i${i}`)),
});
const inMode = (mode: LaunchMode, extra: Partial<State> = {}): State => ({
  ...initialState(),
  mode,
  ...extra,
});

const tm = ["a/1", "b/2", "c/3"].map((id) => ({
  id,
  providerId: id.slice(0, 1),
  providerLabel: id,
  model: id.slice(2),
  local: false,
}));
const withModes = (selected: string | null, mode: LaunchMode = "query") =>
  inMode(mode, { thinkingModes: { modes: tm, selected } });

const streaming = (extra: Partial<State> = {}): State =>
  inMode("query", { query: { ...initialState().query, status: "streaming" }, ...extra });

describe("reducer", () => {
  it("shown resets to none mode, empty input, and does not search", () => {
    const r = reduce(inMode("find", { input: "x" }), { type: "shown" });
    expect(r.effects).toEqual([]);
    expect(r.state).toMatchObject({ mode: "none", input: "", items: [], visible: true });
  });

  it("text change searches with incremented seq and the mode scope", () => {
    const a = reduce(inMode("commands"), { type: "text", text: "ab" });
    const b = reduce(a.state, { type: "text", text: "abc" });
    expect(a.effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "ab", scope: "commands" },
    ]);
    expect(b.effects).toEqual([
      { type: "search", seq: 2, mode: "command", text: "abc", scope: "commands" },
    ]);
  });

  it("run mode searches command with apps scope; find has no scope", () => {
    expect(reduce(inMode("run"), { type: "text", text: "a" }).effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "a", scope: "apps" },
    ]);
    expect(reduce(inMode("find"), { type: "text", text: "a" }).effects).toEqual([
      { type: "search", seq: 1, mode: "find", text: "a", scope: undefined },
    ]);
  });

  it("plain text in none mode searches nothing and keeps the hints", () => {
    const r = reduce(initialState(), { type: "text", text: "abc" });
    expect(r.effects).toEqual([]);
    expect(r.state).toMatchObject({ mode: "none", input: "abc", items: [] });
  });

  it("text change in query mode does not search", () => {
    const r = reduce(inMode("query"), { type: "text", text: "hi" });
    expect(r.effects.some((e) => e.type === "search")).toBe(false);
    expect(r.state.input).toBe("hi");
  });

  it("ignores stale results", () => {
    const s = reduce(inMode("commands"), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "results", seq: 0, items: [item("x")] });
    expect(r.state.items).toEqual([]);
  });

  it("accepts current results and resets selection", () => {
    const s = { ...reduce(inMode("commands"), { type: "text", text: "a" }).state, selected: 2 };
    const r = reduce(s, { type: "results", seq: 1, items: [item("x")] });
    expect(r.state.items).toHaveLength(1);
    expect(r.state.selected).toBe(0);
  });

  it("Down wraps to top", () => {
    const s = { ...withItems(inMode("commands")), selected: 2 };
    expect(reduce(s, k("ArrowDown")).state.selected).toBe(0);
  });

  it("Up wraps to bottom", () => {
    expect(reduce(withItems(inMode("commands")), k("ArrowUp")).state.selected).toBe(2);
  });

  it("arrows with no items do nothing", () => {
    expect(reduce(inMode("commands"), k("ArrowDown")).state.selected).toBe(0);
  });

  it("Enter launches selected", () => {
    const s = { ...withItems(inMode("commands")), selected: 1 };
    expect(reduce(s, k("Enter")).effects).toEqual([{ type: "launch", itemId: "i1", admin: false }]);
  });

  it("Enter with no items does nothing", () => {
    expect(reduce(inMode("commands"), k("Enter")).effects).toEqual([]);
  });

  it("Ctrl+Enter reveals location", () => {
    const r = reduce(withItems(inMode("commands")), k("Enter", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i0" }]);
  });

  it("Ctrl+Shift+Enter launches as admin in Command", () => {
    const r = reduce(withItems(inMode("commands")), k("Enter", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([{ type: "launch", itemId: "i0", admin: true }]);
  });

  it("Ctrl+Shift+Enter ignored in Find", () => {
    const r = reduce(withItems(inMode("find")), k("Enter", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([]);
  });

  it("Ctrl+C copies path in Find", () => {
    const r = reduce(withItems(inMode("find")), k("c", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "copyPath", itemId: "i0" }]);
  });

  it("Ctrl+C ignored with text selection", () => {
    const r = reduce(withItems(inMode("find")), k("c", { ctrl: true, hasSelection: true }));
    expect(r.effects).toEqual([]);
  });

  it("Ctrl+C ignored outside Find", () => {
    expect(reduce(withItems(inMode("commands")), k("c", { ctrl: true })).effects).toEqual([]);
  });

  it.each(["Tab", "Shift+Tab"])("%s does not change mode", (name) => {
    const s = inMode("commands");
    const r = reduce(s, k("Tab", { shift: name !== "Tab" }));
    expect(r.state).toBe(s);
    expect(r.effects).toEqual([]);
  });

  it.each([
    [">", "commands", "command", "commands"],
    ["!", "run", "command", "apps"],
    ["/", "find", "find", undefined],
  ] as const)("prefix %s enters %s, is consumed, and searches", (ch, mode, wire, scope) => {
    const r = reduce(initialState(), { type: "text", text: ch });
    expect(r.state.mode).toBe(mode);
    expect(r.state.input).toBe("");
    expect(r.effects).toEqual([{ type: "search", seq: 1, mode: wire, text: "", scope }]);
  });

  it("? enters query without searching", () => {
    const r = reduce(initialState(), { type: "text", text: "?" });
    expect(r.state).toMatchObject({ mode: "query", input: "" });
    expect(r.effects).toEqual([{ type: "loadThinkingModes" }]);
  });

  it("prefix table is one-to-one", () => {
    expect(Object.values(MODES).map((m) => m.prefix)).toEqual([">", "!", "/", "?"]);
  });

  it("a prefix typed on an empty field in another mode switches mode", () => {
    expect(reduce(inMode("find"), { type: "text", text: ">" }).state.mode).toBe("commands");
  });

  it("prefix not at position 0 is kept as text", () => {
    const s = reduce(inMode("commands"), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "text", text: "a/" });
    expect(r.state.mode).toBe("commands");
    expect(r.state.input).toBe("a/");
  });

  it.each(["/", "?"])("prefix %s for a disabled mode stays as text", (ch) => {
    const s = initialState(["command"]);
    const r = reduce(s, { type: "text", text: ch });
    expect(r.state).toMatchObject({ mode: "none", input: ch });
    expect(r.effects).toEqual([]);
  });

  it("Backspace on empty input inside a mode returns to none", () => {
    const r = reduce(inMode("find"), k("Backspace"));
    expect(r.state.mode).toBe("none");
    expect(r.effects).toEqual([]);
  });

  it("Backspace with text stays in the mode; in none it does nothing", () => {
    const s = inMode("find", { input: "a" });
    expect(reduce(s, k("Backspace")).state.mode).toBe("find");
    expect(reduce(initialState(), k("Backspace")).effects).toEqual([]);
  });

  it("Backspace leaving a streaming ask cancels it", () => {
    const r = reduce(streaming(), k("Backspace"));
    expect(r.state.mode).toBe("none");
    expect(r.effects).toEqual([{ type: "cancelAsk", token: 0 }]);
  });

  it("none mode: arrows move over the hint rows and Enter enters the highlighted mode", () => {
    const s = reduce(initialState(), k("ArrowDown")).state;
    expect(s.selected).toBe(1);
    const r = reduce(s, k("Enter"));
    expect(r.state.mode).toBe("run");
    expect(r.effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "", scope: "apps" },
    ]);
  });

  it("none mode: hints skip disabled modes", () => {
    const s = initialState(["command"]);
    expect(reduce(s, k("ArrowUp")).state.selected).toBe(1);
    expect(reduce({ ...s, selected: 1 }, k("Enter")).state.mode).toBe("run");
  });

  it("enterMode event enters a mode and ignores a disabled one", () => {
    expect(reduce(initialState(), { type: "enterMode", mode: "find" }).state.mode).toBe("find");
    const s = initialState(["command"]);
    expect(reduce(s, { type: "enterMode", mode: "query" }).state).toBe(s);
  });

  it("Ctrl+Shift+Enter skips a program row with no Admin action", () => {
    const prog = { ...item("p"), payload: { program: true } };
    const s = { ...inMode("run"), items: [prog] };
    expect(reduce(s, k("Enter", { ctrl: true, shift: true })).effects).toEqual([]);
    const withAdmin = { ...s, items: [{ ...prog, actions: [{ type: "admin" } as const] }] };
    expect(reduce(withAdmin, k("Enter", { ctrl: true, shift: true })).effects).toEqual([
      { type: "launch", itemId: "p", admin: true },
    ]);
  });

  it("Query Enter submits the question with effort", () => {
    const s = inMode("query", { input: " why? " });
    const r = reduce(s, k("Enter"));
    expect(r.effects[0]).toEqual({ type: "ask", token: 1, text: "why?", effort: "fast" });
    expect(r.state.query.status).toBe("streaming");
  });

  it("Query Enter with empty input does nothing", () => {
    expect(reduce(inMode("query"), k("Enter")).effects).toEqual([]);
  });

  it("Esc ladder: cancel stream, then clear, then hide", () => {
    let s = inMode("query", {
      input: "q",
      query: { ...initialState().query, status: "streaming" },
    });
    let r = reduce(s, k("Escape"));
    expect(r.effects).toEqual([{ type: "cancelAsk", token: 0 }]);
    expect(r.state.input).toBe("q");
    s = r.state;
    r = reduce(s, k("Escape"));
    expect(r.state.input).toBe("");
    expect(r.effects.some((e) => e.type === "hide")).toBe(false);
    r = reduce(r.state, k("Escape"));
    expect(r.effects).toEqual([{ type: "hide" }]);
  });

  it("Esc clearing input keeps the mode and re-searches", () => {
    const r = reduce(inMode("commands", { input: "ab" }), k("Escape"));
    expect(r.state.mode).toBe("commands");
    expect(r.effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "", scope: "commands" },
    ]);
  });

  it("Ctrl+T toggles Fast/Think in Query only", () => {
    const a = reduce(inMode("query"), k("t", { ctrl: true })).state;
    expect(a.query.effort).toBe("think");
    expect(reduce(a, k("t", { ctrl: true })).state.query.effort).toBe("fast");
    expect(reduce(inMode("commands"), k("t", { ctrl: true })).state.query.effort).toBe("fast");
  });

  it("Ctrl+P cycles thinking modes like Tab, in Query only", () => {
    const s = inMode("query", { thinkingModes: { modes: tm, selected: "a/1" } });
    expect(reduce(s, k("p", { ctrl: true })).effects).toEqual([
      { type: "setThinkingMode", id: "b/2" },
    ]);
    expect(reduce(inMode("commands"), k("p", { ctrl: true })).effects).toEqual([]);
  });

  it("ask events build the answer and finish", () => {
    let s = streaming();
    s = reduce(s, {
      type: "ask",
      token: 0,
      event: { type: "started", provider: "p", model: "m" },
    }).state;
    s = reduce(s, { type: "ask", token: 0, event: { type: "delta", text: "He" } }).state;
    s = reduce(s, { type: "ask", token: 0, event: { type: "delta", text: "llo" } }).state;
    s = reduce(s, { type: "ask", token: 0, event: { type: "done", provider: "p" } }).state;
    expect(s.query).toMatchObject({ status: "done", text: "Hello", provider: "p" });
  });

  it("ask error sets error state; no_provider opens settings", () => {
    const r = reduce(streaming(), {
      type: "ask",
      token: 0,
      event: { type: "error", code: "no_provider", message: "none" },
    });
    expect(r.state.query.status).toBe("error");
    expect(r.effects).toContainEqual({ type: "openSettings", section: "providers" });
  });

  it("late delta after Esc is ignored", () => {
    const s = reduce(streaming(), k("Escape")).state;
    const r = reduce(s, { type: "ask", token: 0, event: { type: "delta", text: "old" } });
    expect(r.state.query.text).toBe("");
  });

  it("late done after hidden is ignored", () => {
    const s = reduce(streaming(), { type: "hidden" }).state;
    const r = reduce(s, { type: "ask", token: 0, event: { type: "done", provider: "p" } });
    expect(r.state.query.status).toBe("idle");
  });

  it("hidden while streaming emits cancelAsk", () => {
    expect(reduce(streaming(), { type: "hidden" }).effects).toEqual([
      { type: "cancelAsk", token: 0 },
    ]);
  });

  it("second question cancels the first and ignores its events", () => {
    const r = reduce(streaming({ input: "next" }), k("Enter"));
    expect(r.effects.slice(0, 2)).toEqual([
      { type: "cancelAsk", token: 0 },
      { type: "ask", token: 1, text: "next", effort: "fast" },
    ]);
    const late = reduce(r.state, { type: "ask", token: 0, event: { type: "delta", text: "x" } });
    expect(late.state.query.text).toBe("");
  });

  it("needsConfirm then Enter on same item confirms", () => {
    let s = withItems(inMode("commands"), 1);
    s = reduce(s, {
      type: "outcome",
      commandId: "i0",
      outcome: { type: "needsConfirm", token: "t", expiresMs: 3000 },
    }).state;
    const r = reduce(s, k("Enter"));
    expect(r.effects).toEqual([{ type: "confirm", commandId: "i0", token: "t", kill: false }]);
    expect(r.state.pendingConfirm).toBeNull();
  });

  it("clicking a guarded item twice launches then confirms", () => {
    let s = withItems(inMode("commands"), 3);
    const first = reduce(s, { type: "activate", index: 1 });
    expect(first.state.selected).toBe(1);
    expect(first.effects).toEqual([{ type: "launch", itemId: "i1", admin: false }]);
    s = reduce(first.state, {
      type: "outcome",
      commandId: "i1",
      outcome: { type: "needsConfirm", token: "t", expiresMs: 3000 },
    }).state;
    const second = reduce(s, { type: "activate", index: 1 });
    expect(second.effects).toEqual([{ type: "confirm", commandId: "i1", token: "t", kill: false }]);
  });

  it("activate with ctrl reveals the clicked row", () => {
    const r = reduce(withItems(inMode("commands"), 3), { type: "activate", index: 2, ctrl: true });
    expect(r.state.selected).toBe(2);
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i2" }]);
  });

  it("showItems outcome replaces the list", () => {
    const r = reduce(inMode("commands"), {
      type: "outcome",
      commandId: "x",
      outcome: { type: "showItems", items: [item("a")] },
    });
    expect(r.state.items).toHaveLength(1);
  });

  it("hidden resets state but keeps seq so stale results stay stale", () => {
    const s = reduce(inMode("commands"), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "hidden" });
    expect(r.state.input).toBe("");
    expect(r.state.seq).toBe(1);
  });
});

describe("query state flags", () => {
  const ask = (s: State, event: import("../types").AskEvent) =>
    reduce(s, { type: "ask", token: s.askToken, event }).state.query;

  it("thinking is set by event and cleared by the first delta", () => {
    const t = ask(streaming(), { type: "thinking" });
    expect(t.thinking).toBe(true);
    const s = streaming({ query: t });
    expect(ask(s, { type: "delta", text: "a" }).thinking).toBe(false);
  });

  it("started stores model and reasoning, defaulting reasoning to true", () => {
    const a = ask(streaming(), { type: "started", provider: "p", model: "m", reasoning: false });
    expect(a).toMatchObject({ model: "m", reasoning: false });
    expect(ask(streaming(), { type: "started", provider: "p", model: "m" }).reasoning).toBe(true);
  });

  it("error stores errorCode", () => {
    const e = ask(streaming(), { type: "error", code: "no_provider", message: "x" });
    expect(e.errorCode).toBe("no_provider");
  });

  describe("Backspace kill", () => {
    const procs = (): State => ({
      ...inMode("commands"),
      items: [
        { ...item("p0"), kind: "process" },
        { ...item("p1"), kind: "process" },
      ],
    });

    it("edits text until an arrow moved the selection", () => {
      expect(reduce(procs(), k("Backspace")).effects).toEqual([]);
    });

    it("after an arrow it asks to kill, then Enter confirms the kill", () => {
      const armed = reduce(procs(), k("ArrowDown")).state;
      const r = reduce(armed, k("Backspace"));
      expect(r.effects).toEqual([{ type: "kill", itemId: "p1" }]);
      const pending = reduce(r.state, {
        type: "outcome",
        commandId: "p1",
        outcome: { type: "needsConfirm", token: "t", expiresMs: 3000 },
      }).state;
      expect(reduce(pending, k("Enter")).effects).toEqual([
        { type: "confirm", commandId: "p1", token: "t", kill: true },
      ]);
    });

    it("ignores non-process rows", () => {
      const s = reduce(withItems(inMode("commands")), k("ArrowDown")).state;
      expect(reduce(s, k("Backspace")).effects).toEqual([]);
    });
  });
});

describe("explain attachment", () => {
  const att = (o: Partial<Attachment> = {}): Attachment => ({
    preview: "some text",
    length: 9,
    sourceApp: "notepad.exe",
    blocked: false,
    reason: null,
    ...o,
  });
  const captured = (attachment: Attachment | null, s = initialState()) =>
    reduce(s, { type: "captured", attachment }).state;

  it("captured opens Query mode with the text attached and sends nothing", () => {
    const r = reduce(initialState(), { type: "captured", attachment: att() });
    expect(r.state.mode).toBe("query");
    expect(r.state.attachment?.preview).toBe("some text");
    expect(r.effects).toEqual([{ type: "loadThinkingModes" }]);
  });

  it("nothing selected opens empty Query mode with a notice", () => {
    const s = captured(null);
    expect(s.mode).toBe("query");
    expect(s.attachment).toBeNull();
    expect(s.notice).toBe("No text selected");
  });

  it("ignores a capture while Query mode is off", () => {
    const s = initialState(["command", "find"]);
    expect(captured(att(), s).attachment).toBeNull();
  });

  it("Enter sends the default question with the attached text", () => {
    const r = reduce(captured(att()), k("Enter"));
    expect(r.effects).toEqual([
      { type: "ask", token: 1, text: "Explain this", attachment: "some text", effort: "fast" },
    ]);
  });

  it("Enter sends the typed question when there is one", () => {
    const s = { ...captured(att()), input: "why?" };
    expect(reduce(s, k("Enter")).effects).toMatchObject([{ type: "ask", text: "why?" }]);
  });

  it("blocked text: Enter emits no ask effect", () => {
    const s = { ...captured(att({ blocked: true, reason: "secret", preview: "" })), input: "x" };
    const r = reduce(s, k("Enter"));
    expect(r.effects).toEqual([]);
    expect(r.state.query.status).toBe("idle");
  });

  it("remove clears the attachment", () => {
    const s = reduce(captured(att()), { type: "removeAttachment" }).state;
    expect(s.attachment).toBeNull();
    expect(reduce(s, k("Enter")).effects).toEqual([]);
  });

  it("hidden and shown clear it", () => {
    expect(reduce(captured(att()), { type: "hidden" }).state.attachment).toBeNull();
    expect(reduce(captured(att()), { type: "shown" }).state.attachment).toBeNull();
  });

  it("switching mode clears it", () => {
    expect(
      reduce(captured(att()), { type: "enterMode", mode: "find" }).state.attachment,
    ).toBeNull();
  });
});

describe("thinking modes", () => {
  it("Tab cycles forward and wraps, Shift+Tab back, each persisted", () => {
    let r = reduce(withModes("a/1"), k("Tab"));
    expect(r.state.thinkingModes.selected).toBe("b/2");
    expect(r.effects).toEqual([{ type: "setThinkingMode", id: "b/2" }]);
    expect(reduce(withModes("c/3"), k("Tab")).state.thinkingModes.selected).toBe("a/1");
    r = reduce(withModes("a/1"), k("Tab", { shift: true }));
    expect(r.state.thinkingModes.selected).toBe("c/3");
  });

  it("is a no-op with one mode, none, or outside query", () => {
    const one = inMode("query", { thinkingModes: { modes: tm.slice(0, 1), selected: "a/1" } });
    expect(reduce(one, k("Tab")).effects).toEqual([]);
    expect(reduce(inMode("query"), k("Tab")).effects).toEqual([]);
    expect(reduce(withModes("a/1", "find"), k("Tab")).effects).toEqual([]);
  });

  it("ask carries the selected thinkingMode", () => {
    const r = reduce({ ...withModes("b/2"), input: "hi" }, k("Enter"));
    expect(r.effects).toContainEqual(expect.objectContaining({ type: "ask", thinkingMode: "b/2" }));
  });

  it("entering query loads modes; the event stores them", () => {
    expect(reduce(initialState(), { type: "enterMode", mode: "query" }).effects).toContainEqual({
      type: "loadThinkingModes",
    });
    const r = reduce(initialState(), { type: "thinkingModes", modes: tm, selected: "c/3" });
    expect(r.state.thinkingModes.selected).toBe("c/3");
  });
});

describe("entering modes with text", () => {
  it("Enter on a hint carries typed plain text and searches it", () => {
    const s = { ...initialState(), input: "notepad", selected: 1 };
    const r = reduce(s, {
      type: "key",
      key: { key: "Enter", ctrl: false, shift: false, alt: false },
    });
    expect(r.state.mode).toBe("run");
    expect(r.state.input).toBe("notepad");
    expect(r.effects[0]).toMatchObject({ type: "search", text: "notepad" });
  });
  it("a pasted prefixed string enters that mode with the rest", () => {
    const r = reduce(initialState(), { type: "text", text: "!spot" });
    expect(r.state.mode).toBe("run");
    expect(r.state.input).toBe("spot");
    expect(reduce(initialState(), { type: "text", text: "?why" }).state.mode).toBe("query");
  });
});
