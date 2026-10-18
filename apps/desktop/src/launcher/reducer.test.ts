import { describe, expect, it } from "vitest";
import type { Item, Mode } from "../types";
import { type Event, initialState, type KeyInput, reduce, type State } from "./reducer";

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
const inMode = (mode: Mode, extra: Partial<State> = {}): State => ({
  ...initialState(),
  mode,
  ...extra,
});

describe("reducer", () => {
  it("empty command input on shown searches most-used", () => {
    const r = reduce(initialState(), { type: "shown" });
    expect(r.effects).toEqual([{ type: "search", seq: 1, mode: "command", text: "" }]);
    expect(r.state.visible).toBe(true);
  });

  it("text change searches with incremented seq", () => {
    const a = reduce(initialState(), { type: "text", text: "ab" });
    const b = reduce(a.state, { type: "text", text: "abc" });
    expect(a.effects).toEqual([{ type: "search", seq: 1, mode: "command", text: "ab" }]);
    expect(b.effects).toEqual([{ type: "search", seq: 2, mode: "command", text: "abc" }]);
  });

  it("text change in query mode does not search", () => {
    const r = reduce(inMode("query"), { type: "text", text: "hi" });
    expect(r.effects.some((e) => e.type === "search")).toBe(false);
    expect(r.state.input).toBe("hi");
  });

  it("ignores stale results", () => {
    const s = reduce(initialState(), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "results", seq: 0, items: [item("x")] });
    expect(r.state.items).toEqual([]);
  });

  it("accepts current results and resets selection", () => {
    const s = { ...reduce(initialState(), { type: "text", text: "a" }).state, selected: 2 };
    const r = reduce(s, { type: "results", seq: 1, items: [item("x")] });
    expect(r.state.items).toHaveLength(1);
    expect(r.state.selected).toBe(0);
  });

  it("Down wraps to top", () => {
    const s = { ...withItems(initialState()), selected: 2 };
    expect(reduce(s, k("ArrowDown")).state.selected).toBe(0);
  });

  it("Up wraps to bottom", () => {
    expect(reduce(withItems(initialState()), k("ArrowUp")).state.selected).toBe(2);
  });

  it("arrows with no items do nothing", () => {
    expect(reduce(initialState(), k("ArrowDown")).state.selected).toBe(0);
  });

  it("Enter launches selected", () => {
    const s = { ...withItems(initialState()), selected: 1 };
    expect(reduce(s, k("Enter")).effects).toEqual([{ type: "launch", itemId: "i1", admin: false }]);
  });

  it("Enter with no items does nothing", () => {
    expect(reduce(initialState(), k("Enter")).effects).toEqual([]);
  });

  it("Ctrl+Enter reveals location", () => {
    const r = reduce(withItems(initialState()), k("Enter", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i0" }]);
  });

  it("Ctrl+Shift+Enter launches as admin in Command", () => {
    const r = reduce(withItems(initialState()), k("Enter", { ctrl: true, shift: true }));
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
    expect(reduce(withItems(initialState()), k("c", { ctrl: true })).effects).toEqual([]);
  });

  it("Tab cycles command, find, query, command", () => {
    let s = initialState();
    const seen: Mode[] = [];
    for (let i = 0; i < 3; i++) {
      s = reduce(s, k("Tab")).state;
      seen.push(s.mode);
    }
    expect(seen).toEqual(["find", "query", "command"]);
  });

  it("Shift+Tab cycles backwards", () => {
    expect(reduce(initialState(), k("Tab", { shift: true })).state.mode).toBe("query");
  });

  it("Tab skips disabled Find", () => {
    const s = { ...initialState(), enabledModes: ["command", "query"] as Mode[] };
    expect(reduce(s, k("Tab")).state.mode).toBe("query");
  });

  it("Command stays reachable when enabledModes omits it", () => {
    const s = { ...initialState(), enabledModes: [] as Mode[] };
    expect(reduce(s, k("Tab")).state.mode).toBe("command");
  });

  it("Tab to a search mode searches with empty text", () => {
    const r = reduce(initialState(), k("Tab"));
    expect(r.effects).toEqual([{ type: "search", seq: 1, mode: "find", text: "" }]);
  });

  it.each([
    ["/", "find"],
    ["?", "query"],
  ] as const)("prefix %s as first char switches to %s and is removed", (ch, mode) => {
    const r = reduce(initialState(), { type: "text", text: ch });
    expect(r.state.mode).toBe(mode);
    expect(r.state.input).toBe("");
  });

  it("> switches back to Command", () => {
    const r = reduce(inMode("find"), { type: "text", text: ">" });
    expect(r.state.mode).toBe("command");
    expect(r.state.input).toBe("");
  });

  it("prefix not at position 0 is kept as text", () => {
    const s = reduce(initialState(), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "text", text: "a/" });
    expect(r.state.mode).toBe("command");
    expect(r.state.input).toBe("a/");
  });

  it("prefix for a disabled mode stays as text", () => {
    const s = { ...initialState(), enabledModes: ["command"] as Mode[] };
    const r = reduce(s, { type: "text", text: "/" });
    expect(r.state.mode).toBe("command");
    expect(r.state.input).toBe("/");
  });

  it("Query Enter submits the question with effort", () => {
    const s = inMode("query", { input: " why? " });
    const r = reduce(s, k("Enter"));
    expect(r.effects[0]).toEqual({ type: "ask", text: "why?", effort: "fast" });
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
    expect(r.effects).toEqual([{ type: "cancelAsk" }]);
    expect(r.state.input).toBe("q");
    s = r.state;
    r = reduce(s, k("Escape"));
    expect(r.state.input).toBe("");
    expect(r.effects.some((e) => e.type === "hide")).toBe(false);
    r = reduce(r.state, k("Escape"));
    expect(r.effects).toEqual([{ type: "hide" }]);
  });

  it("Esc clearing in Command re-searches most-used", () => {
    const r = reduce({ ...initialState(), input: "ab" }, k("Escape"));
    expect(r.effects).toEqual([{ type: "search", seq: 1, mode: "command", text: "" }]);
  });

  it("Ctrl+T toggles Fast/Think in Query only", () => {
    const a = reduce(inMode("query"), k("t", { ctrl: true })).state;
    expect(a.query.effort).toBe("think");
    expect(reduce(a, k("t", { ctrl: true })).state.query.effort).toBe("fast");
    expect(reduce(initialState(), k("t", { ctrl: true })).state.query.effort).toBe("fast");
  });

  it("Ctrl+P emits nextProvider in Query only", () => {
    expect(reduce(inMode("query"), k("p", { ctrl: true })).effects).toEqual([
      { type: "nextProvider" },
    ]);
    expect(reduce(initialState(), k("p", { ctrl: true })).effects).toEqual([]);
  });

  it("ask events build the answer and finish", () => {
    let s = inMode("query");
    s = reduce(s, { type: "ask", event: { type: "started", provider: "p", model: "m" } }).state;
    s = reduce(s, { type: "ask", event: { type: "delta", text: "He" } }).state;
    s = reduce(s, { type: "ask", event: { type: "delta", text: "llo" } }).state;
    s = reduce(s, { type: "ask", event: { type: "done", provider: "p" } }).state;
    expect(s.query).toMatchObject({ status: "done", text: "Hello", provider: "p" });
  });

  it("ask error sets error state; no_provider opens settings", () => {
    const r = reduce(inMode("query"), {
      type: "ask",
      event: { type: "error", code: "no_provider", message: "none" },
    });
    expect(r.state.query.status).toBe("error");
    expect(r.effects).toContainEqual({ type: "openSettings", section: "providers" });
  });

  it("needsConfirm then Enter on same item confirms", () => {
    let s = withItems(initialState(), 1);
    s = reduce(s, {
      type: "outcome",
      commandId: "i0",
      outcome: { type: "needsConfirm", token: "t", expiresMs: 3000 },
    }).state;
    const r = reduce(s, k("Enter"));
    expect(r.effects).toEqual([{ type: "confirm", commandId: "i0", token: "t" }]);
    expect(r.state.pendingConfirm).toBeNull();
  });

  it("showItems outcome replaces the list", () => {
    const r = reduce(initialState(), {
      type: "outcome",
      commandId: "x",
      outcome: { type: "showItems", items: [item("a")] },
    });
    expect(r.state.items).toHaveLength(1);
  });

  it("hidden resets state but keeps seq so stale results stay stale", () => {
    const s = reduce(initialState(), { type: "text", text: "a" }).state;
    const r = reduce(s, { type: "hidden" });
    expect(r.state.input).toBe("");
    expect(r.state.seq).toBe(1);
  });
});
