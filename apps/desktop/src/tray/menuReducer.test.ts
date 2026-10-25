import { describe, expect, it } from "vitest";
import { buildItems, type MenuState, reduceMenu } from "./menuReducer";

const start = (update: { version: string } | null = null): MenuState => ({
  items: buildItems({ hotkey: "Alt+Space", update }),
  selected: 0,
});
const key = (state: MenuState, k: string) => reduceMenu(state, { type: "key", key: k });

describe("buildItems", () => {
  it("lists open, settings, update, quit with caps", () => {
    const items = buildItems({ hotkey: "Alt+Space", update: null });
    expect(items.map((i) => i.action)).toEqual(["open", "settings", "update", "quit"]);
    expect(items[0]?.caps).toEqual(["Alt", "Space"]);
    expect(items[1]?.caps).toEqual(["Ctrl", ","]);
    expect(items[2]).toMatchObject({ label: "check for updates", caps: [] });
  });

  it("names the version when an update is present", () => {
    expect(buildItems({ hotkey: "Alt+Space", update: { version: "0.0.2" } })[2]?.label).toBe(
      "install 0.0.2",
    );
  });
});

describe("reduceMenu", () => {
  it("wraps ArrowDown and ArrowUp", () => {
    expect(key(start(), "ArrowUp").state.selected).toBe(3);
    expect(key({ ...start(), selected: 3 }, "ArrowDown").state.selected).toBe(0);
    expect(key(start(), "ArrowDown").state.selected).toBe(1);
  });

  it("jumps with Home and End", () => {
    expect(key({ ...start(), selected: 2 }, "Home").state.selected).toBe(0);
    expect(key(start(), "End").state.selected).toBe(3);
  });

  it("runs the selected action on Enter and Space", () => {
    const s = { ...start(), selected: 1 };
    expect(key(s, "Enter").effects).toEqual([{ type: "run", action: "settings" }]);
    expect(key(s, " ").effects).toEqual([{ type: "run", action: "settings" }]);
  });

  it("closes on Escape and swallows Tab", () => {
    expect(key(start(), "Escape").effects).toEqual([{ type: "close" }]);
    expect(key(start(), "Tab")).toEqual({ state: start(), effects: [] });
  });

  it("hover sets the index and ignores out-of-range", () => {
    expect(reduceMenu(start(), { type: "hover", index: 2 }).state.selected).toBe(2);
    expect(reduceMenu(start(), { type: "hover", index: 9 }).state.selected).toBe(0);
  });
});
