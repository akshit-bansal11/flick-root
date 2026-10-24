import { describe, expect, it } from "vitest";
import type { ActionKind, Item, ItemKind } from "../types";
import { detailOf, keyWords } from "./detail";

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

  it("takes a project's path from its subtitle", () => {
    const p = make("project", { subtitle: "D:/code/p" });
    expect(detailOf(p).facts).toEqual([{ label: "path", value: "D:/code/p" }]);
    expect(detailOf(make("project")).facts).toEqual([]);
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
    expect(d.actions.map((a) => a.id)).toEqual(["open", "location", "admin", "copyPath"]);
    expect(d.actions.map((a) => a.keys)).toEqual([
      "enter",
      "ctrl+enter",
      "ctrl+shift+enter",
      "ctrl+shift+c",
    ]);
  });

  it("labels the first action open and marks only it primary", () => {
    const d = detailOf(make("app", { actions: all }));
    expect(d.actions[0]).toEqual({ id: "open", label: "open", keys: "enter", primary: true });
    expect(d.actions.filter((a) => a.primary)).toHaveLength(1);
  });

  it("labels a command's primary action run", () => {
    const actions: ActionKind[] = [{ type: "primary" }, { type: "copyResult" }];
    const d = detailOf(make("command", { actions }));
    expect(d.actions[0]).toEqual({ id: "run", label: "run", keys: "enter", primary: true });
    expect(d.actions[1]?.id).toBe("copyResult");
  });

  it("labels a custom primary action run", () => {
    const d = detailOf(make("app", { actions: [{ type: "custom", value: "x" }] }));
    expect(d.actions).toEqual([{ id: "run", label: "run", keys: "enter", primary: true }]);
  });

  it("drops a non-first custom action", () => {
    const item = make("app", { actions: [{ type: "primary" }, { type: "custom", value: "x" }] });
    expect(ids(item)).toEqual(["open"]);
  });

  it("hides kill until the process is armed", () => {
    const item = make("process", { actions: [{ type: "primary" }, { type: "kill" }] });
    expect(ids(item)).toEqual(["open"]);
    expect(ids(item, { armedProcess: false })).toEqual(["open"]);
    expect(ids(item, { armedProcess: true })).toEqual(["open", "kill"]);
    expect(detailOf(item, { armedProcess: true }).actions[1]?.keys).toBe("delete");
  });

  it("lists no actions for an item with none", () => {
    expect(detailOf(make("app")).actions).toEqual([]);
  });

  it("does not list a later primary action", () => {
    expect(ids(make("app", { actions: [{ type: "location" }, { type: "primary" }] }))).toEqual([
      "location",
    ]);
  });
});
