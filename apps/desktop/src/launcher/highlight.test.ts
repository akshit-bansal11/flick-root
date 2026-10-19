import { describe, expect, it } from "vitest";
import { highlight } from "./highlight";

describe("highlight", () => {
  it("returns one plain segment for an empty query", () => {
    expect(highlight("Chrome", "")).toEqual([{ text: "Chrome", hit: false, at: 0 }]);
  });

  it("prefers a contiguous match, case-insensitively", () => {
    expect(highlight("Google Chrome", "chr")).toEqual([
      { text: "Google ", hit: false, at: 0 },
      { text: "Chr", hit: true, at: 7 },
      { text: "ome", hit: false, at: 10 },
    ]);
  });

  it("falls back to a greedy subsequence", () => {
    expect(highlight("Visual Studio", "vs")).toEqual([
      { text: "V", hit: true, at: 0 },
      { text: "i", hit: false, at: 1 },
      { text: "s", hit: true, at: 2 },
      { text: "ual Studio", hit: false, at: 3 },
    ]);
  });

  it("marks nothing when the query is not a subsequence", () => {
    expect(highlight("Notepad", "xyz")).toEqual([{ text: "Notepad", hit: false, at: 0 }]);
  });
});
