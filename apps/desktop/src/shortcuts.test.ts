import { describe, expect, it } from "vitest";
import {
  capsOf,
  capsText,
  chordOf,
  groups,
  idFor,
  inScope,
  isReserved,
  keyWords,
  matches,
  parseChord,
  RESERVED,
  type Scope,
  SHORTCUTS,
  scopesOf,
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
    for (const s of SHORTCUTS) {
      for (const scope of scopesOf(s)) {
        expect(isReserved(s.keys, scope), `${s.id} ${s.keys} in ${scope}`).toBe(false);
      }
    }
  });

  it("reserves the WebView2 chords, and lets Settings keep Ctrl+W and Ctrl+Tab", () => {
    for (const c of [
      "Ctrl+F",
      "Ctrl+R",
      "F5",
      "F12",
      "Ctrl+0",
      "Ctrl+=",
      "Ctrl+-",
      "Ctrl+P",
      "Alt+Left",
      "Alt+Space",
    ]) {
      expect(isReserved(c), c).toBe(true);
    }
    expect(isReserved("Ctrl+W")).toBe(true);
    expect(isReserved("Ctrl+W", "launcher")).toBe(true);
    expect(isReserved("Ctrl+W", "settings")).toBe(false);
    expect(isReserved("Ctrl+Tab", "settings")).toBe(false);
  });

  it("lets the launcher own Ctrl+Tab and Ctrl+Shift+Tab, and nothing else reserved", () => {
    expect(isReserved("Ctrl+Tab", "launcher")).toBe(false);
    expect(isReserved("Ctrl+Shift+Tab", "launcher")).toBe(false);
    expect(isReserved("Ctrl+Tab")).toBe(true);
    expect(isReserved("Ctrl+Shift+Tab")).toBe(true);
    expect(isReserved("Ctrl+W", "launcher")).toBe(true);
    expect(isReserved("Ctrl+W", "settings")).toBe(false);
    expect(isReserved("Ctrl+Tab", "settings")).toBe(false);
    expect(isReserved("Ctrl+Shift+Tab", "settings")).toBe(false);
  });

  it("reads Ctrl+digit and Ctrl+/ from the physical key (AZERTY)", () => {
    expect(chordOf(key("&", { ctrl: true, code: "Digit1" }))).toBe("Ctrl+1");
    expect(chordOf(key(":", { ctrl: true, code: "Slash" }))).toBe("Ctrl+/");
    expect(chordOf(key("&", { code: "Digit1" }))).toBe("&");
  });

  it("reserved list covers the editing and Windows chords", () => {
    for (const c of ["Ctrl+Backspace", "Ctrl+Left", "Alt+F4", "Ctrl+Shift+Esc", "Win+L"]) {
      expect(isReserved(c)).toBe(true);
    }
    expect(RESERVED).toContain("Ctrl+Alt+Delete");
  });

  it("Show location applies to Open and plain-text results", () => {
    expect(SHORTCUTS.find((s) => s.id === "location")?.scope).toEqual(["none", "find"]);
  });

  it("Use theme for its variant only is Ctrl+Enter in Commands, where Show location is not", () => {
    const slot = SHORTCUTS.find((s) => s.id === "themeSlot");
    expect(slot?.keys).toBe("Ctrl+Enter");
    expect(slot?.scope).toBe("commands");
    const chord = key("Enter", { ctrl: true });
    expect(idFor("commands", chord)).toBe("themeSlot");
    expect(idFor("find", chord)).toBe("location");
  });

  it("plain-text scope shares no chord with the launcher scope", () => {
    const own = inScope("none").map((s) => s.keys);
    const shared = inScope("launcher").map((s) => s.keys);
    expect(own.filter((c) => shared.includes(c))).toEqual([]);
  });

  it("has unique ids", () => {
    expect(new Set(SHORTCUTS.map((s) => s.id)).size).toBe(SHORTCUTS.length);
  });

  it("shares no chord inside one effective scope", () => {
    const modes: Scope[] = ["none", "find", "commands", "calc", "query", "projects"];
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
    expect(idFor("none", key("Enter"))).toBe("open");
    expect(idFor("commands", key("1", { ctrl: true }))).toBe("go:find");
    expect(idFor("settings", key("w", { ctrl: true }))).toBe("closeSettingsW");
    expect(idFor("find", key("Backspace", { ctrl: true }))).toBeUndefined();
  });
});

describe("mode tab cycling", () => {
  it("maps Ctrl+Tab and Ctrl+Shift+Tab to tabNext and tabPrev in every launcher mode", () => {
    for (const mode of ["none", "find", "commands", "calc", "query", "projects"] as const) {
      expect(idFor(mode, key("Tab", { ctrl: true })), mode).toBe("tabNext");
      expect(idFor(mode, key("Tab", { ctrl: true, shift: true })), mode).toBe("tabPrev");
    }
  });

  it("keeps Tab alone on the model shortcut in the query scope", () => {
    expect(idFor("query", key("Tab"))).toBe("model");
    expect(idFor("query", key("Tab", { shift: true }))).toBe("modelPrev");
  });

  it("keeps Settings page cycling on the same chords", () => {
    expect(idFor("settings", key("Tab", { ctrl: true }))).toBe("pageNext");
    expect(idFor("settings", key("Tab", { ctrl: true, shift: true }))).toBe("pagePrev");
  });
});

describe("keyWords and capsOf", () => {
  it("writes chords as lower-case words", () => {
    expect(keyWords("Ctrl+Shift+Enter")).toBe("ctrl+shift+enter");
    expect(keyWords("Shift+Tab")).toBe("shift+tab");
    expect(keyWords("Alt+Space")).toBe("alt+space");
    expect(keyWords("Esc")).toBe("esc");
    expect(keyWords("F1")).toBe("f1");
    expect(keyWords("Ctrl+,")).toBe("ctrl+,");
  });

  it("keeps key names in their own case for aria-keyshortcuts, as words not glyphs", () => {
    expect(capsOf("Enter")).toEqual(["Enter"]);
    expect(capsOf("Backspace")).toEqual(["Backspace"]);
    expect(capsOf("Up")).toEqual(["Up"]);
    expect(capsOf("Down")).toEqual(["Down"]);
    expect(capsOf("Ctrl+Shift+Enter")).toEqual(["Ctrl", "Shift", "Enter"]);
  });

  it("capsText reads a shortcut as lower-case words", () => {
    expect(capsText("copyPath")).toBe("ctrl shift c");
  });

  it("joins caps into the same words as the chord", () => {
    expect(keyWords(capsOf("Ctrl+Backspace").join("+"))).toBe("ctrl+backspace");
  });
});

describe("tour scope", () => {
  it("binds PageDown and PageUp, neither reserved, listed last in the sheet", () => {
    expect(inScope("tour").map((s) => s.keys)).toEqual(["PageDown", "PageUp"]);
    expect(groups().at(-1)?.scope).toBe("tour");
  });
});
