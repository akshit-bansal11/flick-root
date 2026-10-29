import { describe, expect, it } from "vitest";
import type { ThemeView } from "../theme";
import type { Item, LaunchMode } from "../types";
import { groupLabel, isPinnable, PINNED_GROUP, themeRow } from "./kinds";
import {
  type Attachment,
  availableModes,
  COPIED_MS,
  type Event,
  initialState,
  type KeyInput,
  MODES,
  PLAIN_SEARCH,
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
const located = (id: string): Item => ({
  ...item(id),
  actions: [{ type: "primary" }, { type: "location" }],
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
  it("shown resets to none mode, empty input, and loads home without searching", () => {
    const r = reduce(inMode("find", { input: "x" }), { type: "shown" });
    expect(r.effects).toEqual([{ type: "loadHome" }]);
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

  it("plain text in none mode searches apps and features and bumps seq", () => {
    const a = reduce(initialState(), { type: "text", text: "abc" });
    const { wire: mode, sources } = PLAIN_SEARCH;
    expect(a.effects).toEqual([{ type: "search", seq: 1, mode, text: "abc", sources }]);
    expect(sources).toEqual(["apps", "features"]);
    expect(a.state).toMatchObject({ mode: "none", input: "abc", seq: 1 });
    expect(reduce(a.state, { type: "text", text: "abcd" }).state.seq).toBe(2);
  });

  it("blank text in none mode does not search and stale results are dropped", () => {
    const typed = reduce(initialState(), { type: "text", text: "ab" }).state;
    const cleared = reduce(typed, { type: "text", text: "" });
    expect(cleared.effects).toEqual([{ type: "loadHome" }]);
    expect(cleared.state).toMatchObject({ mode: "none", input: "", items: [] });
    expect(reduce(cleared.state, { type: "text", text: "  " }).effects).toEqual([]);
    const late = reduce(cleared.state, { type: "results", seq: 1, items: [item("x")] });
    expect(late.state.items).toEqual([]);
  });

  describe("fallback rows", () => {
    const live = ["apps", "files", "ask", "syscmd"];
    const plain = (sources: string[] | null, webTitle: string | null, x: Partial<State> = {}) =>
      typedResults({ ...initialState(sources, webTitle), ...x });
    const typedResults = (s: State): State => {
      const typed = reduce(s, { type: "text", text: "cats" }).state;
      return reduce(typed, { type: "results", seq: typed.seq, items: [item("app:cats")] }).state;
    };

    it("come after the results, in order, in the Search instead group", () => {
      const s = plain(live, "Google");
      expect(s.items.map((i) => i.id)).toEqual(["app:cats", "ui:find", "ui:web", "ui:ask"]);
      expect(s.items.slice(1).map((i) => i.title)).toEqual([
        "Find files named “cats”",
        "Search Google for “cats”",
        "Ask your AI: “cats”",
      ]);
      expect(s.items.slice(1).every((i) => i.source === "ui")).toBe(true);
      expect(groupLabel(s.items[1] ?? item("none"))).toBe("Search instead");
    });

    it("show at once while the search is pending, then keep after results", () => {
      const typed = reduce(initialState(live, "Google"), { type: "text", text: "cats" }).state;
      expect(typed.pending).toBe(true);
      expect(typed.items.map((i) => i.id)).toEqual(["ui:find", "ui:web", "ui:ask"]);
      const done = reduce(typed, { type: "results", seq: typed.seq, items: [] }).state;
      expect(done.pending).toBe(false);
      expect(done.items.map((i) => i.id)).toEqual(["ui:find", "ui:web", "ui:ask"]);
    });

    it("drop the Web row without a web title and the Ask row without a live ask", () => {
      expect(plain(live, null).items.map((i) => i.id)).toEqual(["app:cats", "ui:find", "ui:ask"]);
      expect(plain(["apps", "syscmd"], "Google").items.map((i) => i.id)).toEqual([
        "app:cats",
        "ui:find",
        "ui:web",
      ]);
      expect(plain(null, null).items.map((i) => i.id)).toEqual(["app:cats", "ui:find", "ui:ask"]);
    });

    it("name the selected thinking mode in the Ask row", () => {
      const s = plain(live, null, { thinkingModes: { modes: tm, selected: "b/2" } });
      expect(s.items.at(-1)?.title).toBe("Ask b/2 · 2: “cats”");
    });

    it("arrows count the items in plain mode", () => {
      const s = plain(live, "Google");
      expect(reduce(s, k("ArrowUp")).state.selected).toBe(3);
      expect(reduce(s, k("ArrowDown")).state.selected).toBe(1);
    });

    it("Enter on Find enters find carrying the text", () => {
      const r = reduce({ ...plain(live, "Google"), selected: 1 }, k("Enter"));
      expect(r.state).toMatchObject({ mode: "find", input: "cats" });
      expect(r.effects).toEqual([
        { type: "search", seq: 2, mode: "find", text: "cats", sources: MODES.find.sources },
      ]);
    });

    it("Enter on Web emits webSearch with the trimmed text", () => {
      const s = plain(live, "Google");
      const r = reduce({ ...s, input: " cats ", selected: 2 }, k("Enter"));
      expect(r.effects).toEqual([{ type: "webSearch", text: "cats" }]);
    });

    it("Enter on Ask switches to Ask and sends the text", () => {
      const r = reduce({ ...plain(live, "Google"), selected: 3 }, k("Enter"));
      expect(r.state).toMatchObject({ mode: "query", input: "cats" });
      expect(r.state.query.status).toBe("streaming");
      expect(r.effects).toEqual([
        { type: "loadThinkingModes" },
        { type: "ask", token: 1, text: "cats", effort: "fast" },
      ]);
    });

    it("Ctrl+Enter and Ctrl+Shift+Enter do nothing on a fallback row", () => {
      const s = { ...plain(live, "Google"), selected: 2 };
      expect(reduce(s, k("Enter", { ctrl: true })).effects).toEqual([]);
      expect(reduce(s, k("Enter", { ctrl: true, shift: true })).effects).toEqual([]);
    });

    it("Esc clears the text and returns to the prefix list", () => {
      const r = reduce(plain(live, "Google"), k("Escape"));
      expect(r.state).toMatchObject({ mode: "none", input: "", items: [] });
      expect(r.effects).toEqual([{ type: "loadHome" }]);
    });

    it("a prefix as first character still enters its mode", () => {
      expect(reduce(initialState(live), { type: "text", text: "/c" }).state.mode).toBe("find");
    });

    it("the enabledSources event stores webTitle and loads thinking modes once", () => {
      const ev: Event = { type: "enabledSources", sources: live, webTitle: "Bing" };
      const r = reduce(initialState(), ev);
      expect(r.state.webTitle).toBe("Bing");
      expect(r.effects).toEqual([{ type: "loadThinkingModes" }]);
      const loaded = { ...initialState(), thinkingModes: { modes: tm, selected: null } };
      expect(reduce(loaded, ev).effects).toEqual([]);
      expect(reduce(r.state, { type: "shown" }).state.webTitle).toBe("Bing");
    });
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

  it("ignores results in query mode", () => {
    const s = reduce(inMode("commands"), { type: "text", text: "a" }).state;
    const asking = { ...s, mode: "query" as const, items: [] };
    const r = reduce(asking, { type: "results", seq: asking.seq, items: [item("x")] });
    expect(r.state.items).toEqual([]);
  });

  it("searchFailed clears pending for the current search only", () => {
    const s = reduce(inMode("commands"), { type: "text", text: "a" }).state;
    expect(s.pending).toBe(true);
    expect(reduce(s, { type: "searchFailed", seq: s.seq }).state.pending).toBe(false);
    expect(reduce(s, { type: "searchFailed", seq: s.seq - 1 }).state.pending).toBe(true);
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
    const r = reduce({ ...inMode("find"), items: [located("i0")] }, k("Enter", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i0" }]);
  });

  it("Ctrl+Enter does nothing on a row without a location action", () => {
    expect(reduce(withItems(inMode("none")), k("Enter", { ctrl: true })).effects).toEqual([]);
    expect(reduce(withItems(inMode("find")), k("Enter", { ctrl: true })).effects).toEqual([]);
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

  const resultRow = (): Item => ({
    ...item("r"),
    kind: "result",
    actions: [{ type: "copyResult" }],
    payload: { value: "42" },
  });

  it("Enter copies a result value in Calc, Commands and All; a row with no value launches", () => {
    for (const mode of ["calc", "commands", "none"] as const) {
      const s = { ...inMode(mode), items: [resultRow()] };
      expect(reduce(s, k("Enter")).effects, mode).toEqual([{ type: "copyText", text: "42" }]);
    }
    const plain = withItems(inMode("commands"));
    expect(reduce(plain, k("Enter")).effects).toEqual([
      { type: "launch", itemId: "i0", admin: false },
    ]);
  });

  it("Ctrl+Shift+C does nothing on a result row in Calc", () => {
    const s = { ...inMode("calc"), items: [resultRow()] };
    expect(reduce(s, k("C", { ctrl: true, shift: true })).effects).toEqual([]);
  });

  it("clicking a result row copies it", () => {
    const s = { ...inMode("calc"), items: [resultRow()] };
    expect(reduce(s, { type: "activate", index: 0 }).effects).toEqual([
      { type: "copyText", text: "42" },
    ]);
  });

  it("Ctrl+Shift+C copies the Ask answer once there is one", () => {
    const answered = inMode("query", { query: { ...initialState().query, text: "hi there" } });
    expect(reduce(answered, k("C", { ctrl: true, shift: true })).effects[0]).toEqual({
      type: "copyText",
      text: "hi there",
    });
    expect(reduce(inMode("query"), k("C", { ctrl: true, shift: true })).effects).toEqual([]);
  });

  it("Ctrl+1..4 enter modes, carry typed text and respect disabled modes", () => {
    const typed = { ...initialState(), input: "abc" };
    const r = reduce(typed, k("1", { ctrl: true }));
    expect(r.state).toMatchObject({ mode: "find", input: "abc" });
    expect(r.effects[0]).toMatchObject({ type: "search", mode: "find", text: "abc" });
    expect(reduce(typed, k("2", { ctrl: true })).state.mode).toBe("commands");
    expect(reduce(typed, k("3", { ctrl: true })).state.mode).toBe("calc");
    expect(reduce(typed, k("4", { ctrl: true })).state.mode).toBe("query");
    const off = initialState(["syscmd"]);
    expect(reduce(off, k("1", { ctrl: true })).state).toBe(off);
    expect(reduce(off, k("4", { ctrl: true })).state).toBe(off);
  });

  it("a prefix is offered only while one of its needs is live", () => {
    expect(availableModes(initialState(["apps"]).enabledSources)).toEqual(["find"]);
    expect(availableModes(["apps", "syscmd"])).toEqual(["find", "commands"]);
    expect(availableModes(null)).toEqual(["find", "commands", "calc", "query"]);
  });

  it("? stays text when ask is not live", () => {
    const r = reduce(initialState(["apps", "syscmd"]), { type: "text", text: "?" });
    expect(r.state).toMatchObject({ mode: "none", input: "?" });
    expect(reduce(initialState(["apps", "ask"]), { type: "text", text: "?" }).state.mode).toBe(
      "query",
    );
  });

  it("the enabledSources event updates the hint list and survives shown and hidden", () => {
    const ev: Event = { type: "enabledSources", sources: ["apps", "math"], webTitle: null };
    const s = reduce(initialState(), ev).state;
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
    [">", "commands", "command", ["syscmd", "web", "media", "ports", "capture", "links"]],
    ["=", "calc", "command", ["math"]],
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
    expect(Object.values(MODES).map((m) => m.prefix)).toEqual(["/", ">", "=", "?"]);
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
    expect(r.effects).toEqual([expect.objectContaining({ type: "search", text: ch })]);
  });

  it("Backspace on empty input inside a mode returns to none", () => {
    const r = reduce(inMode("find"), k("Backspace"));
    expect(r.state.mode).toBe("none");
    expect(r.effects).toEqual([{ type: "loadHome" }]);
  });

  it("Backspace with text stays in the mode; in none it does nothing", () => {
    const s = inMode("find", { input: "a" });
    expect(reduce(s, k("Backspace")).state.mode).toBe("find");
    expect(reduce(initialState(), k("Backspace")).effects).toEqual([]);
  });

  it("Backspace leaving a streaming ask cancels it", () => {
    const r = reduce(streaming(), k("Backspace"));
    expect(r.state.mode).toBe("none");
    expect(r.effects).toEqual([{ type: "cancelAsk", token: 0 }, { type: "loadHome" }]);
  });

  it("blank all tab shows home and Enter launches the first home row", () => {
    const home = [item("h0"), item("h1")];
    const s = reduce(initialState(), { type: "homeLoaded", items: home, pins: [] }).state;
    expect(s.items).toEqual(home);
    expect(s.home).toEqual(home);
    expect(reduce(s, k("Enter")).effects).toEqual([{ type: "launch", itemId: "h0", admin: false }]);
    const down = reduce(s, k("ArrowDown")).state;
    expect(down.selected).toBe(1);
    expect(reduce(down, k("Enter")).effects).toEqual([
      { type: "launch", itemId: "h1", admin: false },
    ]);
  });

  it("typing a prefix selects its tab and keeps the rest of the text", () => {
    const r = reduce(initialState(), { type: "text", text: ">ab" });
    expect(r.state).toMatchObject({ mode: "commands", input: "ab" });
    expect(r.effects).toEqual([
      { type: "search", seq: 1, mode: "command", text: "ab", sources: MODES.commands.sources },
    ]);
  });

  it("the tab event sets the mode and keeps the typed text", () => {
    const typed = reduce(initialState(), { type: "text", text: "ab" }).state;
    const r = reduce(typed, { type: "tab", mode: "find" });
    expect(r.state).toMatchObject({ mode: "find", input: "ab" });
    expect(r.effects).toContainEqual(expect.objectContaining({ type: "search", text: "ab" }));
  });

  it("entering home from a non-home state requests loadHome; staying on home does not", () => {
    const typed = inMode("none", { input: "ab" });
    expect(reduce(typed, { type: "text", text: "" }).effects).toContainEqual({ type: "loadHome" });
    expect(reduce(inMode("find"), { type: "tab", mode: "none" }).effects).toContainEqual({
      type: "loadHome",
    });
    expect(reduce(initialState(), { type: "text", text: " " }).effects).not.toContainEqual({
      type: "loadHome",
    });
  });

  it("the tab event accepts none, restoring home on a blank field", () => {
    const home = [item("h0")];
    const s = inMode("find", { home });
    const r = reduce(s, { type: "tab", mode: "none" });
    expect(r.state).toMatchObject({ mode: "none", input: "", items: home });
    expect(r.effects).toEqual([{ type: "loadHome" }]);
  });

  it("the tab event ignores a disabled mode and the current mode", () => {
    const s = initialState(["apps"]);
    expect(reduce(s, { type: "tab", mode: "query" }).state).toBe(s);
    expect(reduce(s, { type: "tab", mode: "none" }).state).toBe(s);
  });

  it("Ctrl+Tab cycles the tabs with all first and wraps", () => {
    let s = initialState();
    const seen: LaunchMode[] = [];
    for (let i = 0; i < 5; i++) {
      s = reduce(s, k("Tab", { ctrl: true })).state;
      seen.push(s.mode);
    }
    expect(seen).toEqual(["find", "commands", "calc", "query", "none"]);
  });

  it("Ctrl+Shift+Tab cycles backwards and wraps from all to the last tab", () => {
    const back = k("Tab", { ctrl: true, shift: true });
    expect(reduce(initialState(), back).state.mode).toBe("query");
    expect(reduce(inMode("find"), back).state.mode).toBe("none");
    expect(reduce(inMode("calc"), back).state.mode).toBe("commands");
  });

  it("the tab cycle skips disabled modes", () => {
    const s = initialState(["apps", "math"]);
    const next = reduce(s, k("Tab", { ctrl: true })).state;
    expect(next.mode).toBe("find");
    expect(reduce(next, k("Tab", { ctrl: true })).state.mode).toBe("calc");
    const last = inMode("calc", { enabledSources: ["apps", "math"] });
    expect(reduce(last, k("Tab", { ctrl: true })).state.mode).toBe("none");
  });

  it("a tab switch away from a streaming ask cancels it", () => {
    const r = reduce(streaming({ askToken: 4 }), { type: "tab", mode: "none" });
    expect(r.effects).toContainEqual({ type: "cancelAsk", token: 4 });
    expect(r.state.query.status).toBe("idle");
    const cycled = reduce(streaming({ askToken: 4 }), k("Tab", { ctrl: true }));
    expect(cycled.effects).toContainEqual({ type: "cancelAsk", token: 4 });
  });

  it("results arriving while on home are ignored", () => {
    const home = [item("h0")];
    const s = inMode("none", { home, items: home, seq: 3 });
    expect(reduce(s, { type: "results", seq: 3, items: [item("x")] }).state).toBe(s);
  });

  it("shown requests home exactly once and keeps the loaded home rows", () => {
    const home = [item("h0")];
    const r = reduce(inMode("find", { input: "x", home }), { type: "shown" });
    expect(r.effects.filter((e) => e.type === "loadHome")).toHaveLength(1);
    expect(r.state).toMatchObject({ mode: "none", input: "", home, items: home });
    expect(reduce(inMode("find", { home }), { type: "hidden" }).state.home).toEqual(home);
  });

  it("homeLoaded clamps the selection and leaves other tabs' rows alone", () => {
    const five = Array.from({ length: 5 }, (_, i) => item(`o${i}`));
    const s = inMode("none", { home: five, items: five, selected: 4 });
    const r = reduce(s, { type: "homeLoaded", items: [item("n0"), item("n1")], pins: [] });
    expect(r.state.selected).toBe(1);
    expect(r.state.items.map((i) => i.id)).toEqual(["n0", "n1"]);
    const empty = reduce(s, { type: "homeLoaded", items: [], pins: [] }).state;
    expect(empty).toMatchObject({ selected: 0, items: [] });
    const away = withItems(inMode("find"));
    const kept = reduce(away, { type: "homeLoaded", items: [item("n0")], pins: [] }).state;
    expect(kept.items).toBe(away.items);
    expect(kept.home.map((i) => i.id)).toEqual(["n0"]);
  });

  it("homeLoaded keeps the selection on the same item when a pin reorders the list", () => {
    const old = [item("a"), item("b"), item("c")];
    const s = inMode("none", { home: old, items: old, selected: 2 });
    const next = [item("c"), item("a"), item("b")];
    const r = reduce(s, { type: "homeLoaded", items: next, pins: ["c"] });
    expect(r.state.selected).toBe(0);
    expect(r.state.items[r.state.selected]?.id).toBe("c");
  });

  it("homeLoaded clamps the index when the selected item is gone", () => {
    const old = [item("a"), item("b"), item("c")];
    const s = inMode("none", { home: old, items: old, selected: 2 });
    const r = reduce(s, { type: "homeLoaded", items: [item("a"), item("b")], pins: [] });
    expect(r.state.selected).toBe(1);
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

  it("activate with ctrl reveals the clicked row when it has a location", () => {
    const s = { ...inMode("commands"), items: [item("i0"), item("i1"), located("i2")] };
    const r = reduce(s, { type: "activate", index: 2, ctrl: true });
    expect(r.state.selected).toBe(2);
    expect(r.effects).toEqual([{ type: "reveal", itemId: "i2" }]);
    expect(reduce(s, { type: "activate", index: 1, ctrl: true }).effects).toEqual([]);
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
      expect(reduce(armed, k("Backspace")).effects).toEqual([{ type: "loadHome" }]);
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
    expect(reduce(captured(att()), { type: "tab", mode: "find" }).state.attachment).toBeNull();
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

  it("thinking modes arriving rebuild the Ask fallback title in plain text", () => {
    const typed = reduce(initialState(["apps", "ask"]), { type: "text", text: "cats" }).state;
    const titles = (s: State) => s.items.filter((i) => i.id === "ui:ask").map((i) => i.title);
    expect(titles(typed)).toEqual(["Ask your AI: “cats”"]);
    const r = reduce(typed, { type: "thinkingModes", modes: tm, selected: "b/2" });
    expect(titles(r.state)).toEqual(["Ask b/2 · 2: “cats”"]);
  });

  it("entering query loads modes; the event stores them", () => {
    expect(reduce(initialState(), { type: "tab", mode: "query" }).effects).toContainEqual({
      type: "loadThinkingModes",
    });
    const r = reduce(initialState(), { type: "thinkingModes", modes: tm, selected: "c/3" });
    expect(r.state.thinkingModes.selected).toBe("c/3");
  });
});

describe("entering modes with text", () => {
  it("Ctrl+1 carries typed plain text into Open and searches it", () => {
    const s = { ...initialState(), input: "notepad", selected: 0 };
    const r = reduce(s, k("1", { ctrl: true }));
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
    expect(r.effects).toEqual([expect.objectContaining({ type: "search", text: "!spot" })]);
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

describe("theme quick pick", () => {
  const ROLES: ThemeView["roles"] = {
    bg: "#000000",
    panel: "#111111",
    surface: "#222222",
    line: "#333333",
    text: "#ffffff",
    muted: "#999999",
    accent: "#3399ff",
    accentText: "#66bbff",
    onAccent: "#000000",
    sel: "#223344",
    ok: "#00cc66",
    warn: "#ffaa00",
    err: "#ff4444",
  };
  const RATIOS: ThemeView["ratios"] = {
    text: 7,
    muted: 4.5,
    accentText: 4.5,
    onAccent: 4.5,
    selText: 7,
    mutedBg: 4.5,
    mutedSel: 4.5,
  };
  const tv = (id: string, name: string, dark: boolean): ThemeView => ({
    id,
    name,
    source: "builtin",
    dark,
    roles: ROLES,
    ratios: RATIOS,
    adjusted: [],
    upstream: null,
  });
  const FLICK = tv("flick-default", "Flick Default", true);
  const DRACULA = tv("dracula", "Dracula", true);
  const LIGHT = tv("flick-default-light", "Flick Default Light", false);
  const THEMES = [FLICK, DRACULA, LIGHT];

  const asked = (activeId = "dracula"): State => {
    const shown = reduce(initialState(), { type: "appearance", activeId }).state;
    return reduce(shown, { type: "text", text: ">theme" }).state;
  };
  const loaded = (): State => reduce(asked(), { type: "themesLoaded", themes: THEMES }).state;
  const rowIds = (s: State) => s.items.map((i) => themeRow(i)?.theme.id);

  it("`> theme` asks for the list once and shows nothing until it lands", () => {
    const shown = reduce(initialState(), { type: "appearance", activeId: "dracula" }).state;
    const r = reduce(shown, { type: "text", text: ">theme" });
    expect(r.effects).toEqual([{ type: "loadThemes" }]);
    expect(r.state).toMatchObject({ mode: "commands", input: "theme", items: [] });
    expect(reduce(r.state, { type: "text", text: "theme d" }).effects).toEqual([]);
  });

  it("is also reached by typing theme in the Commands tab, in any case", () => {
    const r = reduce(inMode("commands"), { type: "text", text: "THEME" });
    expect(r.effects).toEqual([{ type: "loadThemes" }]);
  });

  it("other Commands text still searches", () => {
    const r = reduce(inMode("commands"), { type: "text", text: "themes" });
    expect(r.effects).toEqual([expect.objectContaining({ type: "search" })]);
  });

  it("themesLoaded lists every theme and selects the saved one without previewing", () => {
    const r = reduce(asked(), { type: "themesLoaded", themes: THEMES });
    expect(rowIds(r.state)).toEqual(["flick-default", "dracula", "flick-default-light"]);
    expect(r.state.selected).toBe(1);
    expect(r.state.items.map((i) => themeRow(i)?.current)).toEqual([false, true, false]);
    expect(r.effects).toEqual([]);
  });

  it("filters by name after the word theme and lands on the first match", () => {
    const r = reduce(loaded(), { type: "text", text: "theme flick" });
    expect(rowIds(r.state)).toEqual(["flick-default", "flick-default-light"]);
    expect(r.state.selected).toBe(0);
    expect(r.effects).toEqual([{ type: "previewTheme", theme: FLICK }]);
  });

  it("emits one previewTheme per selection move and none when the row is unchanged", () => {
    const down = reduce(loaded(), k("ArrowDown"));
    expect(down.effects).toEqual([{ type: "previewTheme", theme: LIGHT }]);
    const wrap = reduce(down.state, k("ArrowDown"));
    expect(wrap.effects).toEqual([{ type: "previewTheme", theme: FLICK }]);
    const one = reduce(loaded(), { type: "text", text: "theme dra" }).state;
    expect(rowIds(one)).toEqual(["dracula"]);
    expect(reduce(one, k("ArrowDown")).effects).toEqual([]);
  });

  it("Enter applies the dark theme to its slot and forces the dark scheme", () => {
    const r = reduce(loaded(), k("Enter"));
    expect(r.effects).toEqual([
      { type: "applyTheme", patch: { darkTheme: "dracula", scheme: "dark" } },
    ]);
  });

  it("Enter on a light theme saves the light slot and the light scheme", () => {
    const down = reduce(loaded(), k("ArrowDown")).state;
    expect(reduce(down, k("Enter")).effects).toEqual([
      { type: "applyTheme", patch: { lightTheme: "flick-default-light", scheme: "light" } },
    ]);
  });

  it("a click applies like Enter", () => {
    const r = reduce(loaded(), { type: "activate", index: 0, ctrl: false });
    expect(r.effects).toContainEqual({
      type: "applyTheme",
      patch: { darkTheme: "flick-default", scheme: "dark" },
    });
  });

  it("Ctrl+Enter saves the slot only, leaving the scheme alone", () => {
    const r = reduce(loaded(), k("Enter", { ctrl: true }));
    expect(r.effects).toEqual([{ type: "applyTheme", patch: { darkTheme: "dracula" } }]);
  });

  it("Esc once reverts the preview and keeps the text; Esc twice clears it", () => {
    const previewing = reduce(loaded(), k("ArrowDown")).state;
    const first = reduce(previewing, k("Escape"));
    expect(first.effects).toEqual([{ type: "revertTheme" }]);
    expect(first.state.input).toBe("theme");
    const second = reduce(first.state, k("Escape"));
    expect(second.effects).not.toContainEqual({ type: "revertTheme" });
    expect(second.state.input).toBe("");
  });

  it("Esc without a preview goes straight to clearing the text", () => {
    const r = reduce(loaded(), k("Escape"));
    expect(r.effects).not.toContainEqual({ type: "revertTheme" });
    expect(r.state.input).toBe("");
  });

  it("hidden while previewing reverts; hidden without a preview does not", () => {
    const previewing = reduce(loaded(), k("ArrowDown")).state;
    expect(reduce(previewing, { type: "hidden" }).effects).toEqual([{ type: "revertTheme" }]);
    expect(reduce(loaded(), { type: "hidden" }).effects).toEqual([]);
  });

  it("leaving the pick with a preview reverts, by text or by tab", () => {
    const previewing = reduce(loaded(), k("ArrowDown")).state;
    expect(reduce(previewing, { type: "text", text: "x" }).effects).toContainEqual({
      type: "revertTheme",
    });
    expect(reduce(previewing, { type: "tab", mode: "none" }).effects).toContainEqual({
      type: "revertTheme",
    });
  });

  it("the appearance event moves the current tag to the saved theme", () => {
    const r = reduce(loaded(), { type: "appearance", activeId: "flick-default" });
    expect(r.state.theme.activeId).toBe("flick-default");
    expect(r.state.items.map((i) => themeRow(i)?.current)).toEqual([true, false, false]);
  });

  it("shown starts a fresh pick that asks for the list again", () => {
    const r = reduce(loaded(), { type: "shown" });
    expect(r.state.theme).toMatchObject({ list: null, loading: false, activeId: "dracula" });
  });
});

describe("pins", () => {
  const ctrlP = k("p", { ctrl: true });
  const row = (id: string, home?: string): Item => ({
    ...item(id),
    payload: home ? { home } : null,
  });
  const selecting = (id: string): State => inMode("find", { items: [item(id)], selected: 0 });

  it("Ctrl+P on an app or file row emits togglePin with its id", () => {
    for (const id of ["app:a", "file:C:/x/a.txt"]) {
      expect(reduce(selecting(id), ctrlP).effects).toEqual([{ type: "togglePin", itemId: id }]);
    }
  });

  it("Ctrl+P on the selected row, not the first", () => {
    const s = inMode("find", { items: [item("cmd:x"), item("app:b")], selected: 1 });
    expect(reduce(s, ctrlP).effects).toEqual([{ type: "togglePin", itemId: "app:b" }]);
  });

  it("Ctrl+P on other rows or an empty list does nothing and keeps the state", () => {
    for (const id of ["cmd:x", "ui:find", "theme:dracula"]) {
      const s = selecting(id);
      const r = reduce(s, ctrlP);
      expect(r.effects).toEqual([]);
      expect(r.state).toEqual(s);
    }
    const empty = inMode("find");
    const r = reduce(empty, ctrlP);
    expect(r.effects).toEqual([]);
    expect(r.state).toEqual(empty);
  });

  it("starts with no pins", () => {
    expect(initialState().pins).toEqual([]);
  });

  it("homeLoaded stores pins on the home tab and replaces the rows", () => {
    const items = [row("app:p", "pinned"), row("app:r", "recent")];
    const r = reduce(initialState(), { type: "homeLoaded", items, pins: ["app:p"] });
    expect(r.state.pins).toEqual(["app:p"]);
    expect(r.state.items).toEqual(items);
    expect(r.state.home).toEqual(items);
  });

  it("homeLoaded stores pins on other tabs without touching their rows", () => {
    const away = withItems(inMode("find"));
    const r = reduce(away, { type: "homeLoaded", items: [row("app:p")], pins: ["app:p"] });
    expect(r.state.pins).toEqual(["app:p"]);
    expect(r.state.items).toBe(away.items);
    const cleared = reduce(r.state, { type: "homeLoaded", items: [], pins: [] }).state;
    expect(cleared.pins).toEqual([]);
  });

  it("shown and hidden keep the pins", () => {
    const s = inMode("find", { pins: ["app:p"], input: "x" });
    expect(reduce(s, { type: "shown" }).state.pins).toEqual(["app:p"]);
    expect(reduce(s, { type: "hidden" }).state.pins).toEqual(["app:p"]);
  });
});

describe("kinds: pins", () => {
  it("labels pinned and recent home rows, and falls back to the kind group", () => {
    const tagged = (home: unknown): Item => ({ ...item("app:a"), payload: { home } });
    expect(groupLabel(tagged("pinned"))).toBe(PINNED_GROUP);
    expect(groupLabel(tagged("pinned"))).toBe("pinned");
    expect(groupLabel(tagged("recent"))).toBe("recent");
    expect(groupLabel(tagged("other"))).toBe(groupLabel(item("app:a")));
    expect(groupLabel(tagged("other"))).not.toBe("pinned");
  });

  it("isPinnable accepts app: and file: ids only", () => {
    expect(isPinnable(item("app:a"))).toBe(true);
    expect(isPinnable(item("file:C:/a"))).toBe(true);
    for (const id of ["cmd:x", "ui:find", "theme:dracula", "proj:p", "app", "xapp:a"]) {
      expect(isPinnable(item(id)), id).toBe(false);
    }
  });
});
