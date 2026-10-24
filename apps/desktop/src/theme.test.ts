import { describe, expect, it, vi } from "vitest";
import { type ActiveTheme, applyTheme, ROLE_VARS, ROLES, type ThemeRoles } from "./theme";

const roles = Object.fromEntries(ROLES.map((r, i) => [r, `#0${i.toString(16)}0a0b`])) as ThemeRoles;
const active: ActiveTheme = { id: "t", dark: true, roles };
const el = () => document.createElement("html");

describe("applyTheme", () => {
  it("sets every role var and data-scheme", () => {
    const root = el();
    applyTheme(root, active);
    for (const r of ROLES) expect(root.style.getPropertyValue(ROLE_VARS[r])).toBe(roles[r]);
    expect(root.dataset.scheme).toBe("dark");
    applyTheme(root, { ...active, id: "l", dark: false, roles: { ...roles, bg: "#ffffff" } });
    expect(root.dataset.scheme).toBe("light");
  });

  it("is a no-op on an identical signature", () => {
    const root = el();
    applyTheme(root, active);
    const spy = vi.spyOn(root.style, "setProperty");
    applyTheme(root, { ...active, roles: { ...roles } });
    expect(spy).not.toHaveBeenCalled();
  });

  it("re-applies after a changed role", () => {
    const root = el();
    applyTheme(root, active);
    applyTheme(root, { ...active, roles: { ...roles, accent: "#123456" } });
    expect(root.style.getPropertyValue("--l-accent")).toBe("#123456");
  });

  it("null clears the role vars and falls back to matchMedia", () => {
    const root = el();
    applyTheme(root, active);
    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    applyTheme(root, null);
    vi.unstubAllGlobals();
    for (const r of ROLES) expect(root.style.getPropertyValue(ROLE_VARS[r])).toBe("");
    expect(root.dataset.scheme).toBe("light");
  });
});
