import { describe, expect, it } from "vitest";
import { cancelsDefault, isBrowserChord } from "./keys";

const ev = (key: string, o: KeyboardEventInit = {}) => new KeyboardEvent("keydown", { key, ...o });

describe("cancelsDefault", () => {
  it("cancels a table chord even when the reducer did nothing", () => {
    expect(cancelsDefault(ev("1", { ctrlKey: true }), "commands", false)).toBe(true);
    expect(cancelsDefault(ev("Enter"), "find", false)).toBe(true);
  });

  it("cancels browser chords everywhere", () => {
    for (const [key, o] of [
      ["p", { ctrlKey: true }],
      ["r", { ctrlKey: true }],
      ["F5", {}],
      ["ArrowLeft", { altKey: true }],
      ["-", { ctrlKey: true }],
    ] as const) {
      expect(isBrowserChord(ev(key, o)), key).toBe(true);
      expect(cancelsDefault(ev(key, o), "none", false), key).toBe(true);
    }
  });

  it("leaves Backspace and Delete to the text field unless the reducer acted", () => {
    expect(cancelsDefault(ev("Delete"), "commands", false)).toBe(false);
    expect(cancelsDefault(ev("Delete"), "commands", true)).toBe(true);
    expect(cancelsDefault(ev("Backspace"), "find", false)).toBe(false);
  });

  it("leaves plain typing alone", () => {
    expect(cancelsDefault(ev("a"), "find", false)).toBe(false);
  });
});
