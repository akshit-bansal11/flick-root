import { describe, expect, it } from "vitest";
import type { ThemeView } from "../theme";
import {
  chipCounts,
  filterThemes,
  initPicker,
  PICKER_CHIPS,
  type PickerChip,
  type PickerEffect,
  type PickerKey,
  type PickerState,
  reducePicker,
  visibleThemes,
} from "./themePicker";

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

const theme = (
  id: string,
  name: string,
  source: ThemeView["source"],
  dark: boolean,
): ThemeView => ({
  id,
  name,
  source,
  dark,
  roles: ROLES,
  ratios: RATIOS,
  adjusted: [],
  upstream: null,
});

const THEMES: ThemeView[] = [
  theme("flick-dark", "Flick Dark", "builtin", true),
  theme("flick-light", "Flick Light", "builtin", false),
  theme("dracula", "Dracula", "port", true),
  theme("solarized-light", "Solarized Light", "port", false),
  theme("imported:mine", "My Theme", "imported", true),
];

const ids = (list: ThemeView[]) => list.map((t) => t.id);
const start = (chip: PickerChip = "all", current: string | null = null) =>
  initPicker(THEMES, chip, current);
const key = (s: PickerState, k: PickerKey) => reducePicker(s, { type: "key", key: k });
const previews = (effects: PickerEffect[]) => effects.filter((e) => e.type === "preview");
const NAV_KEYS: PickerKey[] = ["ArrowUp", "ArrowDown", "Home", "End", "Enter"];

describe("filterThemes", () => {
  it("chip all with an empty query keeps input order", () => {
    expect(ids(filterThemes(THEMES, "", "all"))).toEqual(ids(THEMES));
  });

  it("matches the name case-insensitively and keeps input order", () => {
    expect(ids(filterThemes(THEMES, "LIGHT", "all"))).toEqual(["flick-light", "solarized-light"]);
    expect(ids(filterThemes(THEMES, "fLiCk", "all"))).toEqual(["flick-dark", "flick-light"]);
  });

  it("returns nothing for a query that matches no name", () => {
    expect(filterThemes(THEMES, "zzz", "all")).toEqual([]);
  });

  it("applies each chip", () => {
    expect(ids(filterThemes(THEMES, "", "dark"))).toEqual([
      "flick-dark",
      "dracula",
      "imported:mine",
    ]);
    expect(ids(filterThemes(THEMES, "", "light"))).toEqual(["flick-light", "solarized-light"]);
    expect(ids(filterThemes(THEMES, "", "flick"))).toEqual(["flick-dark", "flick-light"]);
    expect(ids(filterThemes(THEMES, "", "ide"))).toEqual(["dracula", "solarized-light"]);
    expect(ids(filterThemes(THEMES, "", "imported"))).toEqual(["imported:mine"]);
  });

  it("combines chip and query", () => {
    expect(ids(filterThemes(THEMES, "light", "ide"))).toEqual(["solarized-light"]);
    expect(filterThemes(THEMES, "dracula", "flick")).toEqual([]);
  });
});

describe("chipCounts", () => {
  it("counts every chip over all themes", () => {
    expect(chipCounts(THEMES)).toEqual({
      all: 5,
      dark: 3,
      light: 2,
      flick: 2,
      ide: 2,
      imported: 1,
    });
  });

  it("has a key per chip and zeros for an empty list", () => {
    const counts = chipCounts([]);
    for (const chip of PICKER_CHIPS) expect(counts[chip]).toBe(0);
  });
});

describe("initPicker and visibleThemes", () => {
  it("keeps currentId when it is visible", () => {
    const s = start("all", "dracula");
    expect(s.selected).toBe("dracula");
    expect(s.query).toBe("");
    expect(s.chip).toBe("all");
  });

  it("falls back to the first visible row when currentId is hidden by the chip", () => {
    expect(start("light", "dracula").selected).toBe("flick-light");
  });

  it("falls back to the first row for null or unknown ids", () => {
    expect(start("ide", null).selected).toBe("dracula");
    expect(start("all", "nope").selected).toBe("flick-dark");
  });

  it("selects null when nothing is visible", () => {
    expect(initPicker([], "all", "dracula").selected).toBeNull();
    expect(initPicker(THEMES.slice(0, 2), "imported", null).selected).toBeNull();
  });

  it("visibleThemes follows query and chip", () => {
    const s: PickerState = { themes: THEMES, query: "light", chip: "ide", selected: null };
    expect(ids(visibleThemes(s))).toEqual(["solarized-light"]);
  });
});

describe("reducePicker navigation", () => {
  it("ArrowDown moves down with one preview", () => {
    const r = key(start(), "ArrowDown");
    expect(r.state.selected).toBe("flick-light");
    expect(r.effects).toEqual([{ type: "preview", id: "flick-light" }]);
  });

  it("ArrowDown wraps from the last row to the first", () => {
    const r = key(start("all", "imported:mine"), "ArrowDown");
    expect(r.state.selected).toBe("flick-dark");
    expect(r.effects).toEqual([{ type: "preview", id: "flick-dark" }]);
  });

  it("ArrowUp moves up and wraps from the first row to the last", () => {
    const up = key(start("all", "dracula"), "ArrowUp");
    expect(up.state.selected).toBe("flick-light");
    expect(up.effects).toEqual([{ type: "preview", id: "flick-light" }]);
    const wrap = key(start(), "ArrowUp");
    expect(wrap.state.selected).toBe("imported:mine");
    expect(wrap.effects).toEqual([{ type: "preview", id: "imported:mine" }]);
  });

  it("Home and End jump to the ends", () => {
    const end = key(start("all", "dracula"), "End");
    expect(end.state.selected).toBe("imported:mine");
    expect(end.effects).toEqual([{ type: "preview", id: "imported:mine" }]);
    const home = key(start("all", "dracula"), "Home");
    expect(home.state.selected).toBe("flick-dark");
    expect(home.effects).toEqual([{ type: "preview", id: "flick-dark" }]);
  });

  it("emits no preview when the selection does not change", () => {
    expect(previews(key(start(), "Home").effects)).toEqual([]);
    expect(previews(key(start("all", "imported:mine"), "End").effects)).toEqual([]);
    const single = initPicker(THEMES.slice(0, 1), "all", null);
    expect(previews(key(single, "ArrowDown").effects)).toEqual([]);
    expect(previews(key(single, "ArrowUp").effects)).toEqual([]);
  });

  it("navigates only within the filtered list", () => {
    const r = key(start("light"), "ArrowDown");
    expect(r.state.selected).toBe("solarized-light");
    expect(key(r.state, "ArrowDown").state.selected).toBe("flick-light");
  });
});

describe("reducePicker Enter and Escape", () => {
  it("Enter applies a dark theme to darkTheme and does not close", () => {
    const r = key(start("all", "dracula"), "Enter");
    expect(r.effects).toContainEqual({ type: "apply", id: "dracula", slot: "darkTheme" });
    expect(r.effects.some((e) => e.type === "close")).toBe(false);
    expect(r.state.selected).toBe("dracula");
  });

  it("Enter applies a light theme to lightTheme", () => {
    const r = key(start("all", "solarized-light"), "Enter");
    expect(r.effects).toContainEqual({ type: "apply", id: "solarized-light", slot: "lightTheme" });
    expect(r.effects.some((e) => e.type === "close")).toBe(false);
  });

  it("Escape reverts then closes", () => {
    const r = key(start("all", "dracula"), "Escape");
    expect(r.effects).toEqual([{ type: "revert" }, { type: "close" }]);
  });
});

describe("reducePicker query and chip", () => {
  it("stores the query and narrows the visible rows", () => {
    const r = reducePicker(start(), { type: "query", query: "solar" });
    expect(r.state.query).toBe("solar");
    expect(ids(visibleThemes(r.state))).toEqual(["solarized-light"]);
    expect(r.state.selected).toBe("solarized-light");
  });

  it("keeps the selection when it is still visible", () => {
    const r = reducePicker(start("all", "flick-light"), { type: "query", query: "light" });
    expect(r.state.selected).toBe("flick-light");
  });

  it("moves the selection to the first row when it is filtered out", () => {
    const r = reducePicker(start("all", "dracula"), { type: "chip", chip: "light" });
    expect(r.state.chip).toBe("light");
    expect(r.state.selected).toBe("flick-light");
  });

  it("an empty result selects null and reverts", () => {
    const r = reducePicker(start(), { type: "query", query: "zzz" });
    expect(r.state.selected).toBeNull();
    expect(r.effects).toContainEqual({ type: "revert" });
  });

  it("an empty chip result selects null and reverts", () => {
    const only = initPicker(THEMES.slice(0, 2), "all", null);
    const r = reducePicker(only, { type: "chip", chip: "imported" });
    expect(r.state.selected).toBeNull();
    expect(r.effects).toContainEqual({ type: "revert" });
  });

  it("navigation and Enter are no-ops on an empty list", () => {
    const empty = reducePicker(start(), { type: "query", query: "zzz" }).state;
    for (const k of NAV_KEYS) {
      const r = key(empty, k);
      expect(r.state.selected).toBeNull();
      expect(r.effects).toEqual([]);
    }
  });
});

describe("reducePicker loaded", () => {
  const loaded = (s: PickerState, themes: ThemeView[]) =>
    reducePicker(s, { type: "loaded", themes });

  it("keeps the selection when the id is still present", () => {
    const r = loaded(start("all", "dracula"), THEMES.slice().reverse());
    expect(r.state.selected).toBe("dracula");
    expect(r.state.themes).toHaveLength(THEMES.length);
  });

  it("falls back to the first visible row when the selection disappeared", () => {
    const r = loaded(
      start("all", "imported:mine"),
      THEMES.filter((t) => t.source !== "imported"),
    );
    expect(r.state.selected).toBe("flick-dark");
  });

  it("selects null when the new list has no visible rows", () => {
    expect(loaded(start(), []).state.selected).toBeNull();
  });

  it("can select a row from a previously empty list", () => {
    expect(loaded(initPicker([], "all", null), THEMES).state.selected).toBe("flick-dark");
  });
});
