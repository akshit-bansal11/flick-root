import { describe, expect, it } from "vitest";
import {
  chordOf,
  groups,
  idFor,
  inScope,
  isReserved,
  matches,
  parseChord,
  RESERVED,
  type Scope,
  SHORTCUTS,
} from "./shortcuts";

const key = (k: string, o: object = {}) => ({
  key: k,
  ctrl: false,
  shift: false,
  alt: false,
  ...o,
});

describe("shortcut table", () => {
  it("has no reserved chord", () => {
    for (const s of SHORTCUTS) expect(isReserved(s.keys), `${s.id} ${s.keys}`).toBe(false);
  });

  it("reserved list covers the editing and Windows chords", () => {
    for (const c of ["Ctrl+Backspace", "Ctrl+Left", "Alt+F4", "Ctrl+Shift+Esc", "Win+L"]) {
      expect(isReserved(c)).toBe(true);
    }
    expect(RESERVED).toContain("Ctrl+Alt+Delete");
  });

  it("has unique ids", () => {
    expect(new Set(SHORTCUTS.map((s) => s.id)).size).toBe(SHORTCUTS.length);
  });

  it("shares no chord inside one effective scope", () => {
    const modes: Scope[] = ["none", "commands", "run", "find", "query"];
    const effective = [...modes.map((m): Scope[] => [m, "launcher"]), ["settings"] as Scope[]];
    for (const scopes of effective) {
      const chords = scopes.flatMap((s) => inScope(s).map((x) => x.keys));
      expect(new Set(chords).size, scopes.join("+")).toBe(chords.length);
    }
  });

  it("writes every chord in canonical form", () => {
    for (const s of SHORTCUTS) expect(chordOf(parseChord(s.keys)), s.id).toBe(s.keys);
  });

  it("groups every entry under a scope", () => {
    expect(groups().flatMap((g) => g.items).length).toBeGreaterThanOrEqual(SHORTCUTS.length);
  });
});

describe("chordOf and matches", () => {
  it("normalizes names and case", () => {
    expect(chordOf(key("C", { ctrl: true, shift: true }))).toBe("Ctrl+Shift+C");
    expect(chordOf(key("c", { ctrl: true }))).toBe("Ctrl+C");
    expect(chordOf(key("Escape"))).toBe("Esc");
    expect(chordOf(key("ArrowUp"))).toBe("Up");
    expect(chordOf(key("Tab", { ctrl: true, shift: true }))).toBe("Ctrl+Shift+Tab");
    expect(matches(key("Enter", { ctrl: true }), "Ctrl+Enter")).toBe(true);
    expect(matches(key("Enter"), "Ctrl+Enter")).toBe(false);
  });

  it("looks up by mode, mode entries first", () => {
    expect(idFor("find", key("Enter"))).toBe("open");
    expect(idFor("query", key("Enter"))).toBe("ask");
    expect(idFor("none", key("Enter"))).toBe("enterMode");
    expect(idFor("commands", key("1", { ctrl: true }))).toBe("go:commands");
    expect(idFor("settings", key("w", { ctrl: true }))).toBe("closeSettingsW");
    expect(idFor("find", key("Backspace", { ctrl: true }))).toBeUndefined();
  });
});
