import { describe, expect, it } from "vitest";
import { applyAppearance, clampGlass, GLASS_MIN, resolveScheme } from "./theme";

const base = { theme: "glass", glass: 62, scheme: "system", glassAvailable: true } as const;

describe("applyAppearance", () => {
  it("sets attributes and alpha", () => {
    const el = document.createElement("html");
    applyAppearance(el, { ...base, glass: 80, systemDark: () => true });
    expect(el.dataset.theme).toBe("glass");
    expect(el.dataset.scheme).toBe("dark");
    expect(el.dataset.glass).toBe("on");
    expect(el.style.getPropertyValue("--l-glass-alpha")).toBe("80%");
  });
  it("resolves system to light", () => {
    const el = document.createElement("html");
    applyAppearance(el, { ...base, systemDark: () => false });
    expect(el.dataset.scheme).toBe("light");
  });
  it("solid and unavailable transparency turn glass off", () => {
    const el = document.createElement("html");
    applyAppearance(el, { ...base, theme: "solid", scheme: "dark" });
    expect(el.dataset.glass).toBe("off");
    applyAppearance(el, { ...base, scheme: "dark", glassAvailable: false });
    expect(el.dataset.glass).toBe("off");
  });
  it("forced themes ignore the scheme", () => {
    expect(resolveScheme({ theme: "frost", glass: 62, scheme: "dark" }, true)).toBe("light");
    expect(resolveScheme({ theme: "ember", glass: 62, scheme: "light" }, false)).toBe("dark");
  });
  it("clamps glass", () => {
    expect(clampGlass(5)).toBe(GLASS_MIN);
    expect(clampGlass(200)).toBe(95);
  });
});
