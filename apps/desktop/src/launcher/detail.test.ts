import { describe, expect, it } from "vitest";
import { keyWords } from "../shortcuts";
import type { ThemeView } from "../theme";
import type { ActionKind, Item, ItemKind } from "../types";
import { detailOf } from "./detail";
import { THEME_SOURCE } from "./kinds";

const make = (kind: ItemKind, o: Partial<Item> = {}): Item => ({
  id: `${kind}:x`,
  kind,
  title: "Thing",
  subtitle: "",
  icon: null,
  score: 1,
  source: "test",
  actions: [],
  payload: null,
  ...o,
});

const ids = (item: Item, ctx = {}) => detailOf(item, ctx).actions.map((a) => a.id);

describe("keyWords", () => {
  it("lowercases a chord", () => {
    expect(keyWords("Ctrl+Shift+Enter")).toBe("ctrl+shift+enter");
  });
});

describe("detailOf facts", () => {
  it("shows an app's parsing name as path", () => {
    const app = make("app", { payload: { parsingName: "C:/Apps/a.exe" } });
    expect(detailOf(app).facts).toEqual([{ label: "path", value: "C:/Apps/a.exe" }]);
    expect(detailOf(app).tag).toBe("app");
  });

  it("shows no facts for an app without a parsing name", () => {
    expect(detailOf(make("app")).facts).toEqual([]);
    expect(detailOf(make("app", { payload: { parsingName: "" } })).facts).toEqual([]);
    expect(detailOf(make("app", { payload: { parsingName: 3 } })).facts).toEqual([]);
  });

  it("tags a drive-scan app as program", () => {
    expect(detailOf(make("app", { payload: { program: true } })).tag).toBe("program");
  });

  it("shows a drive-scan program's folder as its path", () => {
    const prog = make("app", { payload: { program: true }, subtitle: "D:/tools" });
    expect(detailOf(prog).facts).toEqual([{ label: "path", value: "D:/tools" }]);
  });

  it("takes a file or folder path from the id minus file:", () => {
    const file = make("file", { id: "file:C:/a/b.txt" });
    const folder = make("folder", { id: "file:C:/a" });
    expect(detailOf(file).facts).toEqual([{ label: "path", value: "C:/a/b.txt" }]);
    expect(detailOf(file).tag).toBe("file");
    expect(detailOf(folder).facts).toEqual([{ label: "path", value: "C:/a" }]);
    expect(detailOf(folder).tag).toBe("folder");
  });

  it("takes a command or process subtitle as about", () => {
    const c = make("command", { subtitle: "Lock the PC" });
    expect(detailOf(c).facts).toEqual([{ label: "about", value: "Lock the PC" }]);
    expect(detailOf(c).tag).toBe("command");
    expect(detailOf(make("process", { subtitle: "pid 4" })).facts).toEqual([
      { label: "about", value: "pid 4" },
    ]);
    expect(detailOf(make("process")).facts).toEqual([]);
  });

  it("carries the title through", () => {
    expect(detailOf(make("app", { title: "Notepad" })).title).toBe("Notepad");
  });
});

describe("detailOf actions", () => {
  const all: ActionKind[] = [
    { type: "primary" },
    { type: "location" },
    { type: "admin" },
    { type: "copyPath" },
  ];

  it("keeps the item's action order with chords as words", () => {
    const d = detailOf(make("app", { actions: all }));
    expect(d.actions.map((a) => a.id)).toEqual(["open", "location", "admin", "copyPath", "pin"]);
    expect(d.actions.map((a) => a.keys)).toEqual([
      "enter",
      "ctrl+enter",
      "ctrl+shift+enter",
      "ctrl+shift+c",
      "ctrl+p",
    ]);
  });

  it("labels the first action open and marks only it primary", () => {
    const d = detailOf(make("app", { actions: all }));
    expect(d.actions[0]).toEqual({ id: "open", label: "open", keys: "enter", primary: true });
    expect(d.actions.filter((a) => a.primary)).toHaveLength(1);
  });

  it("appends a pin action to apps and files, unpin when pinned, keys from the table", () => {
    const pin = { id: "pin", label: "pin", keys: "ctrl+p", primary: false };
    expect(detailOf(make("app")).actions).toEqual([pin]);
    expect(detailOf(make("file")).actions).toEqual([pin]);
    expect(detailOf(make("file"), { pinned: true }).actions).toEqual([{ ...pin, label: "unpin" }]);
    expect(detailOf(make("app", { actions: all })).actions.at(-1)).toEqual(pin);
  });

  it("adds no pin action to other rows, whatever the pinned flag", () => {
    const primary = [{ type: "primary" } as const];
    expect(ids(make("command", { actions: primary }), { pinned: true })).toEqual(["run"]);
    expect(ids(make("process", { actions: primary }), { pinned: true })).toEqual(["open"]);
    expect(ids(make("command"), { pinned: true })).toEqual([]);
  });

  it("tags a pinned app or file '<kind> · pinned', and only those", () => {
    expect(detailOf(make("file"), { pinned: true }).tag).toBe("file · pinned");
    expect(detailOf(make("app"), { pinned: true }).tag).toBe("app · pinned");
    expect(detailOf(make("app", { payload: { program: true } }), { pinned: true }).tag).toBe(
      "program · pinned",
    );
    expect(detailOf(make("app"), { pinned: false }).tag).toBe("app");
    expect(detailOf(make("command"), { pinned: true }).tag).toBe("command");
  });

  it("labels a command's primary action run", () => {
    const actions: ActionKind[] = [{ type: "primary" }, { type: "copyResult" }];
    const d = detailOf(make("command", { actions }));
    expect(d.actions[0]).toEqual({ id: "run", label: "run", keys: "enter", primary: true });
    expect(d.actions[1]?.id).toBe("open");
    expect(d.actions[1]?.label).toBe("copy result");
  });

  it("a result row's first action is copy result on Enter", () => {
    const d = detailOf(make("command", { actions: [{ type: "copyResult" }] }));
    expect(d.actions[0]).toEqual({
      id: "open",
      label: "copy result",
      keys: "enter",
      primary: true,
    });
  });

  it("labels a custom primary action run", () => {
    const d = detailOf(make("app", { actions: [{ type: "custom", value: "x" }] }));
    expect(d.actions[0]).toEqual({ id: "run", label: "run", keys: "enter", primary: true });
  });

  it("drops a non-first custom action", () => {
    const item = make("app", { actions: [{ type: "primary" }, { type: "custom", value: "x" }] });
    expect(ids(item)).toEqual(["open", "pin"]);
  });

  it("hides kill until the process is armed", () => {
    const item = make("process", { actions: [{ type: "primary" }, { type: "kill" }] });
    expect(ids(item)).toEqual(["open"]);
    expect(ids(item, { armedProcess: false })).toEqual(["open"]);
    expect(ids(item, { armedProcess: true })).toEqual(["open", "kill"]);
    expect(detailOf(item, { armedProcess: true }).actions[1]?.keys).toBe("delete");
  });

  it("lists no actions for an item with none", () => {
    expect(detailOf(make("command")).actions).toEqual([]);
  });

  it("does not list a later primary action", () => {
    const item = make("app", { actions: [{ type: "location" }, { type: "primary" }] });
    expect(ids(item)).toEqual(["location", "pin"]);
  });
});

describe("theme rows", () => {
  const ROLES: ThemeView["roles"] = {
    bg: "#000001",
    panel: "#000002",
    surface: "#000003",
    line: "#000004",
    text: "#000005",
    muted: "#000006",
    accent: "#000007",
    accentText: "#000008",
    onAccent: "#000009",
    sel: "#00000a",
    ok: "#00000b",
    warn: "#00000c",
    err: "#00000d",
  };
  const theme = (o: Partial<ThemeView> = {}): ThemeView => ({
    id: "dracula",
    name: "Dracula",
    source: "port",
    dark: true,
    roles: ROLES,
    ratios: {
      text: 7,
      muted: 4.5,
      accentText: 4.56,
      onAccent: 4.5,
      selText: 7,
      mutedBg: 4.5,
      mutedSel: 4.5,
    },
    adjusted: [],
    upstream: null,
    ...o,
  });
  const row = (t: ThemeView, current = false): Item =>
    make("choice", { source: THEME_SOURCE, title: t.name, payload: { theme: t, current } });

  it("shows scheme, source and contrast, and tags the saved theme current", () => {
    const d = detailOf(row(theme(), true));
    expect(d.title).toBe("Dracula");
    expect(d.tag).toBe("current");
    expect(d.facts).toEqual([
      { label: "scheme", value: "dark" },
      { label: "source", value: "port of an editor theme" },
      { label: "text", value: "7.0:1" },
      { label: "accent", value: "4.6:1" },
    ]);
  });

  it("tags other themes by scheme", () => {
    expect(detailOf(row(theme())).tag).toBe("dark");
    expect(detailOf(row(theme({ dark: false }))).tag).toBe("light");
  });

  it("adds upstream and adjusted facts only when present", () => {
    const labels = (t: ThemeView) => detailOf(row(t)).facts.map((f) => f.label);
    expect(labels(theme())).toEqual(["scheme", "source", "text", "accent"]);
    const more = theme({ upstream: "https://example.com/dracula", adjusted: ["muted", "accent"] });
    expect(labels(more)).toEqual(["scheme", "source", "text", "accent", "upstream", "adjusted"]);
    expect(detailOf(row(more)).facts.at(-1)?.value).toBe("muted, accent");
  });

  it("offers apply on Enter and the variant slot on Ctrl+Enter", () => {
    expect(detailOf(row(theme())).actions).toEqual([
      { id: "open", label: "apply", keys: "enter", primary: true },
      { id: "themeSlot", label: "use as dark only", keys: "ctrl+enter", primary: false },
    ]);
    expect(detailOf(row(theme({ dark: false }))).actions[1]?.label).toBe("use as light only");
  });

  it("carries six role swatches in display order", () => {
    expect(detailOf(row(theme())).swatches).toEqual([
      { role: "bg", color: "#000001" },
      { role: "panel", color: "#000002" },
      { role: "sel", color: "#00000a" },
      { role: "accent", color: "#000007" },
      { role: "text", color: "#000005" },
      { role: "muted", color: "#000006" },
    ]);
  });

  it("other rows have no swatches", () => {
    expect(detailOf(make("app")).swatches).toBeUndefined();
  });
});
