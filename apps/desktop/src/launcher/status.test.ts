import { describe, expect, it } from "vitest";
import type { Item, ItemKind } from "../types";
import { countsOf, countText, statusHints } from "./status";

const make = (kind: ItemKind): Item => ({
  id: `${kind}:x`,
  kind,
  title: "Thing",
  subtitle: "",
  icon: null,
  score: 1,
  source: "test",
  actions: [],
  payload: null,
});

const labels = (s: { hints: { label: string }[] }) => s.hints.map((h) => h.label);

describe("countsOf", () => {
  it("counts per kind in order of first appearance", () => {
    const kinds: ItemKind[] = ["file", "app", "file", "folder", "app", "file"];
    const items = kinds.map(make);
    expect(countsOf(items)).toEqual([
      { kind: "file", n: 3 },
      { kind: "app", n: 2 },
      { kind: "folder", n: 1 },
    ]);
  });

  it("is empty for no items", () => {
    expect(countsOf([])).toEqual([]);
  });
});

describe("countText", () => {
  it("uses singular for 1 and plural otherwise", () => {
    expect(countText([{ kind: "app", n: 2 }])).toBe("2 apps");
    expect(countText([{ kind: "file", n: 1 }])).toBe("1 file");
    expect(countText([{ kind: "process", n: 2 }])).toBe("2 processes");
    expect(
      countText([
        { kind: "app", n: 2 },
        { kind: "file", n: 1 },
      ]),
    ).toBe("2 apps · 1 file");
  });

  it("is empty with no counts", () => {
    expect(countText([])).toBe("");
  });
});

describe("statusHints", () => {
  const counts = countsOf([make("app"), make("app"), make("file")]);

  it("carries the count text and is empty-text with no rows", () => {
    expect(statusHints("find", {}, counts).text).toBe("2 apps · 1 file");
    expect(statusHints("find", {}, []).text).toBe("");
  });

  it("starts with the move hint, then the open hint", () => {
    const s = statusHints("find", {}, counts);
    expect(s.hints[0]).toEqual({ id: "move", keys: "↑↓", label: "move" });
    expect(s.hints[1]).toEqual({ id: "open", keys: "enter", label: "open" });
  });

  it("hides a hint whose condition does not hold", () => {
    expect(labels(statusHints("find", {}, counts))).toEqual(["move", "open"]);
    expect(labels(statusHints("find", { revealable: true }, counts))).toEqual([
      "move",
      "open",
      "location",
    ]);
  });

  it("caps the hints at three", () => {
    const all = { revealable: true, adminAction: true, pathItem: true };
    const s = statusHints("find", all, counts);
    expect(s.hints).toHaveLength(3);
    expect(labels(s)).toEqual(["move", "open", "location"]);
  });

  it("never shows move in query mode", () => {
    const s = statusHints("query", { answer: true }, []);
    expect(labels(s)).not.toContain("move");
    expect(labels(s)).toEqual(["ask", "copy answer", "model"]);
    expect(s.hints.length).toBeLessThanOrEqual(3);
  });
});
