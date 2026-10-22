// Applies the appearance settings to <html>: pure DOM attribute/property writes, no Tauri calls.
export const THEMES = ["glass", "frost", "midnight", "aurora", "ember", "solid"] as const;
export type ThemeId = (typeof THEMES)[number];
export type Scheme = "system" | "dark" | "light";

/** Lowest tint opacity that keeps every text role at 4.5:1 over a white or black backdrop (tokens.contrast.test.ts). */
export const GLASS_MIN = 80;
export const GLASS_MAX = 95;
export const GLASS_DEFAULT = 90;

/** Themes whose palette only works in one scheme. */
export const FORCED_SCHEME: Partial<Record<ThemeId, "dark" | "light">> = {
  frost: "light",
  midnight: "dark",
  aurora: "dark",
  ember: "dark",
};

export interface Appearance {
  theme: ThemeId;
  glass: number;
  scheme: Scheme;
}

export const DEFAULT_APPEARANCE: Appearance = {
  theme: "glass",
  glass: GLASS_DEFAULT,
  scheme: "system",
};

export interface ApplyInput extends Appearance {
  /** False when Windows transparency effects are off. */
  glassAvailable: boolean;
  /** Resolves the system scheme; defaults to the prefers-color-scheme media query. */
  systemDark?: () => boolean;
}

export function isThemeId(v: unknown): v is ThemeId {
  return THEMES.some((t) => t === v);
}

export function clampGlass(n: number): number {
  return Math.min(GLASS_MAX, Math.max(GLASS_MIN, Math.round(n)));
}

/** The dark/light scheme a theme renders in. */
export function resolveScheme(a: Appearance, systemDark: boolean): "dark" | "light" {
  const forced = FORCED_SCHEME[a.theme];
  if (forced) return forced;
  return a.scheme === "system" ? (systemDark ? "dark" : "light") : a.scheme;
}

export function applyAppearance(root: HTMLElement, a: ApplyInput) {
  const systemDark = a.systemDark ?? (() => matchMedia("(prefers-color-scheme: dark)").matches);
  root.dataset.theme = a.theme;
  root.dataset.scheme = resolveScheme(a, a.scheme === "system" && systemDark());
  root.dataset.glass = a.glassAvailable && a.theme !== "solid" ? "on" : "off";
  root.style.setProperty("--l-glass-alpha", `${clampGlass(a.glass)}%`);
}
