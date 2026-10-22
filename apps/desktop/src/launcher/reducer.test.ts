import { describe, expect, it } from "vitest";
import type { Item, LaunchMode } from "../types";
import {
  type Attachment,
  availableModes,
  COPIED_MS,
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

  it("text change searches with incremented seq and the mode sources", () => {
    const a = reduce(inMode("commands"), { type: "text", text: "ab" });
    const b = reduce(a.state, { type: "text", text: "abc" });
    const sources = MODES.commands.sources;
    expect(a.effects).toEqual([{ type: "search", seq: 1, mode: "command", text: "ab", sources }]);
    expect(b.effects).toEqual([{ type: "search", seq: 2, mode: "command", text: "abc", sources }]);
  });

  it("find searches the find wire with apps, files and nlfind", () => {
    expect(reduce(inMode("find"), { type: "text", text: "a" }).effects).toEqual([
      { type: "search", seq: 1, mode: "find", text: "a", sources: ["apps", "files", "nlfind"] },
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
    const r = reduce(withItems(inMode("find")), k("Enter", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i0" }]);
  });

  it("Ctrl+Shift+Enter launches as admin in Open when the row has an admin action", () => {
    const s = withItems(inMode("find"));
    const first: Item = { ...item("i0"), actions: [{ type: "primary" }, { type: "admin" }] };
    const r = reduce({ ...s, items: [first] }, k("Enter", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([{ type: "launch", itemId: "i0", admin: true }]);
  });

  it("Ctrl+Shift+Enter does nothing in Open without an admin action", () => {
    const r = reduce(withItems(inMode("find")), k("Enter", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([]);
  });

  it("Ctrl+Shift+Enter ignored in Commands", () => {
    const r = reduce(withItems(inMode("commands")), k("Enter", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([]);
  });

  it("Ctrl+Shift+C does nothing on a non-file row", () => {
    const r = reduce(withItems(inMode("find")), k("C", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([]);
  });

  it("Ctrl+Shift+C copies the path of a file row; Copied shows only after the copy lands", () => {
    const s = inMode("find", { items: [item("file:i0")] });
    const r = reduce(s, k("C", { ctrl: true, shift: true }));
    expect(r.effects).toEqual([{ type: "copyPath", itemId: "file:i0" }]);
    expect(r.state.copied).toBe(false);
    const done = reduce(r.state, { type: "copyDone", ok: true });
    expect(done.effects).toEqual([{ type: "clearCopied", afterMs: COPIED_MS }]);
    expect(done.state.copied).toBe(true);
    expect(reduce(done.state, { type: "copiedDone" }).state.copied).toBe(false);
  });

  it("a failed copy flags Copy failed, not Copied, and clears it later", () => {
    const r = reduce(inMode("find"), { type: "copyDone", ok: false });
    expect(r.state.copied).toBe(false);
    expect(r.state.copyFailed).toBe(true);
    expect(r.effects).toEqual([{ type: "clearCopied", afterMs: COPIED_MS }]);
    expect(reduce(r.state, { type: "copiedDone" }).state.copyFailed).toBe(false);
  });

  it("plain Ctrl+C is never taken: the input copies natively", () => {
    for (const mode of ["find", "commands", "query"] as const) {
      const s = withItems(inMode(mode));
      const r = reduce(s, k("c", { ctrl: true }));
      expect(r.state).toBe(s);
      expect(r.effects).toEqual([]);
    }
  });

  it("Ctrl+Shift+C copies a Commands result value, and nothing without one", () => {
    const withValue = {
      ...inMode("commands"),
      items: [
        { ...item("r"), actions: [{ type: "copyResult" } as const], payload: { value: "42" } },
      ],
    };
    const r = reduce(withValue, k("C", { ctrl: true, shift: true }));
    expect(r.effects[0]).toEqual({ type: "copyText", text: "42" });
    const plain = withItems(inMode("commands"));
    expect(reduce(plain, k("C", { ctrl: true, shift: true })).effects).toEqual([]);
  });

  it("Ctrl+Shift+C copies the Ask answer once there is one", () => {
    const answered = inMode("query", { query: { ...initialState().query, text: "hi there" } });
    expect(reduce(answered, k("C", { ctrl: true, shift: true })).effects[0]).toEqual({
      type: "copyText",
      text: "hi there",
    });
    expect(reduce(inMode("query"), k("C", { ctrl: true, shift: true })).effects).toEqual([]);
  });

  it("Ctrl+1..5 enter modes, carry typed text and respect disabled modes", () => {
    const typed = { ...initialState(), input: "abc" };
    const r = reduce(typed, k("1", { ctrl: true }));
    expect(r.state).toMatchObject({ mode: "find", input: "abc" });
    expect(r.effects[0]).toMatchObject({ type: "search", mode: "find", text: "abc" });
    expect(reduce(typed, k("2", { ctrl: true })).state.mode).toBe("commands");
    expect(reduce(typed, k("3", { ctrl: true })).state.mode).toBe("calc");
    expect(reduce(typed, k("4", { ctrl: true })).state.mode).toBe("query");
    expect(reduce(typed, k("5", { ctrl: true })).state.mode).toBe("projects");
    const off = initialState(["syscmd"]);
    expect(reduce(off, k("1", { ctrl: true })).state).toBe(off);
    expect(reduce(off, k("4", { ctrl: true })).state).toBe(off);
  });

  it("a prefix is offered only while one of its needs is live", () => {
    expect(availableModes(initialState(["apps"]).enabledSources)).toEqual(["find"]);
    expect(availableModes(["apps", "syscmd"])).toEqual(["find", "commands"]);
    expect(availableModes(null)).toEqual(["find", "commands", "calc", "query", "projects"]);
  });

  it("? stays text when ask is not live", () => {
    const r = reduce(initialState(["apps", "syscmd"]), { type: "text", text: "?" });
    expect(r.state).toMatchObject({ mode: "none", input: "?" });
    expect(reduce(initialState(["apps", "ask"]), { type: "text", text: "?" }).state.mode).toBe(
      "query",
    );
  });

  it("the enabledSources event updates the hint list and survives shown and hidden", () => {
    const s = reduce(initialState(), { type: "enabledSources", sources: ["apps", "math"] }).state;
    expect(s.enabledSources).toEqual(["apps", "math"]);
    expect(availableModes(s.enabledSources)).toEqual(["find", "calc"]);
    expect(reduce(s, { type: "shown" }).state.enabledSources).toEqual(["apps", "math"]);
    expect(reduce(s, { type: "hidden" }).state.enabledSources).toEqual(["apps", "math"]);
  });

  it("Ctrl+, opens Settings", () => {
    expect(reduce(initialState(), k(",", { ctrl: true })).effects).toEqual([
      { type: "openSettings", section: "general" },
    ]);
  });

  describe("shortcut sheet", () => {
    it("F1 and Ctrl+/ toggle it", () => {
      const open = reduce(initialState(), k("F1")).state;
      expect(open.sheet).toBe(true);
      expect(reduce(open, k("F1")).state.sheet).toBe(false);
      expect(
        reduce(reduce(initialState(), k("/", { ctrl: true })).state, k("/", { ctrl: true })).state
          .sheet,
      ).toBe(false);
    });

    it("Esc closes it first, before clearing input or hiding", () => {
      const open = { ...inMode("find", { input: "x" }), sheet: true };
      const r = reduce(open, k("Escape"));
      expect(r.state).toMatchObject({ sheet: false, input: "x" });
      expect(r.effects).toEqual([]);
    });

    it("swallows other keys while open", () => {
      const open = { ...withItems(inMode("commands")), sheet: true };
      expect(reduce(open, k("ArrowDown")).state).toBe(open);
      expect(reduce(open, k("Enter")).effects).toEqual([]);
    });
  });

  it("Ctrl+Shift+X removes the attachment", () => {
    const s = inMode("query", {
      attachment: { preview: "p", length: 1, sourceApp: "", blocked: false, reason: null },
    });
    expect(reduce(s, k("X", { ctrl: true, shift: true })).state.attachment).toBeNull();
  });

  it.each(["Tab", "Shift+Tab"])("%s does not change mode", (name) => {
    const s = inMode("commands");
    const r = reduce(s, k("Tab", { shift: name !== "Tab" }));
    expect(r.state).toBe(s);
    expect(r.effects).toEqual([]);
  });

  it.each([
    ["/", "find", "find", ["apps", "files", "nlfind"]],
    [">", "commands", "command", ["syscmd", "web", "media", "ports", "capture"]],
    ["=", "calc", "command", ["math"]],
    ["@", "projects", "command", ["projects"]],
  ] as const)("prefix %s enters %s, is consumed, and searches", (ch, mode, wire, sources) => {
    const r = reduce(initialState(), { type: "text", text: ch });
    expect(r.state.mode).toBe(mode);
    expect(r.state.input).toBe("");
    expect(r.effects).toEqual([{ type: "search", seq: 1, mode: wire, text: "", sources }]);
  });

  it("? enters query without searching", () => {
    const r = reduce(initialState(), { type: "text", text: "?" });
    expect(r.state).toMatchObject({ mode: "query", input: "" });
    expect(r.effects).toEqual([{ type: "loadThinkingModes" }]);
  });

  it("prefix table is one-to-one", () => {
    expect(Object.values(MODES).map((m) => m.prefix)).toEqual(["/", ">", "=", "?", "@"]);
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
    const s = initialState(["syscmd"]);
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
    expect(r.state.mode).toBe("commands");
    expect(r.effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "", sources: MODES.commands.sources },
    ]);
  });

  it("none mode: hints skip disabled modes", () => {
    const s = initialState(["apps", "syscmd", "math", "projects"]);
    expect(reduce(s, k("ArrowUp")).state.selected).toBe(3);
    expect(reduce({ ...s, selected: 2 }, k("Enter")).state.mode).toBe("calc");
  });

  it("enterMode event enters a mode and ignores a disabled one", () => {
    expect(reduce(initialState(), { type: "enterMode", mode: "find" }).state.mode).toBe("find");
    const s = initialState(["apps"]);
    expect(reduce(s, { type: "enterMode", mode: "query" }).state).toBe(s);
  });

  it("Ctrl+Shift+Enter skips a program row with no Admin action", () => {
    const prog = { ...item("p"), payload: { program: true } };
    const s = { ...inMode("find"), items: [prog] };
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
      { type: "search", seq: 1, mode: "command", text: "", sources: MODES.commands.sources },
    ]);
  });

  it("Ctrl+T toggles Fast/Think in Query only", () => {
    const a = reduce(inMode("query"), k("t", { ctrl: true })).state;
    expect(a.query.effort).toBe("think");
    expect(reduce(a, k("t", { ctrl: true })).state.query.effort).toBe("fast");
    expect(reduce(inMode("commands"), k("t", { ctrl: true })).state.query.effort).toBe("fast");
  });

  it("Ctrl+P is not a thinking-mode key (the browser prints on it)", () => {
    const s = inMode("query", { thinkingModes: { modes: tm, selected: "a/1" } });
    expect(reduce(s, k("p", { ctrl: true })).effects).toEqual([]);
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

  describe("Delete kill", () => {
    const procs = (): State => ({
      ...inMode("commands"),
      items: [
        { ...item("p0"), kind: "process" },
        { ...item("p1"), kind: "process" },
      ],
    });

    it("edits text until an arrow moved the selection", () => {
      expect(reduce(procs(), k("Delete")).effects).toEqual([]);
    });

    it("Left, Right, Home and End disarm, so Delete edits text again", () => {
      for (const key of ["ArrowLeft", "ArrowRight", "Home", "End"]) {
        const armed = reduce(procs(), k("ArrowDown")).state;
        const moved = reduce(armed, k(key)).state;
        expect(moved.armed, key).toBe(false);
        expect(reduce(moved, k("Delete")).effects, key).toEqual([]);
      }
    });

    it("Backspace never kills, even after an arrow", () => {
      const armed = reduce(procs(), k("ArrowDown")).state;
      expect(reduce(armed, k("Backspace")).effects).toEqual([]);
    });

    it("after an arrow Delete asks to kill, then Enter confirms the kill", () => {
      const armed = reduce(procs(), k("ArrowDown")).state;
      const r = reduce(armed, k("Delete"));
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
      expect(reduce(s, k("Delete")).effects).toEqual([]);
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
    const s = initialState(["apps", "files", "syscmd"]);
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
    const s = { ...initialState(), input: "notepad", selected: 0 };
    const r = reduce(s, {
      type: "key",
      key: { key: "Enter", ctrl: false, shift: false, alt: false },
    });
    expect(r.state.mode).toBe("find");
    expect(r.state.input).toBe("notepad");
    expect(r.effects[0]).toMatchObject({ type: "search", text: "notepad" });
  });
  it("a pasted prefixed string enters that mode with the rest", () => {
    const r = reduce(initialState(), { type: "text", text: "/spot" });
    expect(r.state.mode).toBe("find");
    expect(r.state.input).toBe("spot");
    expect(reduce(initialState(), { type: "text", text: "?why" }).state.mode).toBe("query");
  });
  it("the old ! prefix is plain text now", () => {
    const r = reduce(initialState(), { type: "text", text: "!spot" });
    expect(r.state).toMatchObject({ mode: "none", input: "!spot" });
    expect(r.effects).toEqual([]);
  });

  describe("actionFailed", () => {
    const failed = (s: State) => reduce(s, { type: "actionFailed", message: "Sleep failed" }).state;

    it("stores the message as the notice and keeps the launcher open", () => {
      const r = reduce(inMode("commands"), { type: "actionFailed", message: "Sleep failed" });
      expect(r.state.notice).toBe("Sleep failed");
      expect(r.effects).toEqual([]);
    });

    it("typing clears it", () => {
      const s = reduce(failed(inMode("commands")), { type: "text", text: "x" }).state;
      expect(s.notice).toBeNull();
      expect(reduce(failed(inMode("none")), { type: "text", text: "x" }).state.notice).toBeNull();
    });

    it("hide clears it", () => {
      expect(reduce(failed(inMode("commands")), { type: "hidden" }).state.notice).toBeNull();
    });

    it("a successful outcome clears it", () => {
      const r = reduce(failed(inMode("commands")), {
        type: "outcome",
        commandId: "c",
        outcome: { type: "done" },
      });
      expect(r.state.notice).toBeNull();
    });
  });
});
