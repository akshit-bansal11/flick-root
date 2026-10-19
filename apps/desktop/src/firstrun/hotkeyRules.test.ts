import { describe, expect, it } from "vitest";
import { MSG_EDITING, MSG_RESERVED, validateCombo, warnCombo } from "./hotkeyRules";

describe("hotkey OS rules", () => {
  it.each([
    ["Alt+Space", "window menu"],
    ["Super+Space", "input language"],
    ["Ctrl+Space", "IME toggle"],
    ["Alt+Shift", "keyboard layout"],
    ["Ctrl+Escape", "Start menu"],
  ])("warns for %s but does not block it", (combo, text) => {
    expect(warnCombo(combo)).toContain(text);
    if (combo !== "Alt+Shift") expect(validateCombo(combo, "")).toBeNull();
  });

  it("does not warn for an ordinary chord", () => {
    expect(warnCombo("Ctrl+Alt+K")).toBeNull();
  });

  it.each(["Ctrl+Backspace", "Ctrl+Shift+Left", "Ctrl+A", "Alt+Backspace", "Ctrl+Shift+Z"])(
    "blocks the text-editing chord %s",
    (combo) => {
      expect(validateCombo(combo, "")).toBe(MSG_EDITING);
    },
  );

  it("keeps blocking Windows-owned chords", () => {
    expect(validateCombo("Alt+F4", "")).toBe(MSG_RESERVED);
  });
});
