// Applies the resolved theme colours from Rust to an element: pure DOM writes, no Tauri calls.
export type Scheme = "system" | "dark" | "light";

/** The role colours Rust resolves for a theme (hex), in RolesView order. */
export const ROLES = [
  "bg",
  "panel",
  "surface",
  "line",
  "text",
  "muted",
  "accent",
  "accentText",
  "onAccent",
  "sel",
  "ok",
  "warn",
  "err",
] as const;
export type ThemeRole = (typeof ROLES)[number];
export type ThemeRoles = Record<ThemeRole, string>;

/** The custom property each role writes; tokens.css derives every other colour from these. */
export const ROLE_VARS: Record<ThemeRole, string> = {
  bg: "--l-bg",
  panel: "--l-panel",
  surface: "--l-surface",
  line: "--l-line",
  text: "--l-text",
  muted: "--l-muted",
  accent: "--l-accent",
  accentText: "--l-accent-text",
  onAccent: "--l-on-accent",
  sel: "--l-sel",
  ok: "--l-ok",
  warn: "--l-warn",
  err: "--l-err",
};

/** The theme in use for the current scheme; mirrors ActiveTheme in Rust. */
export interface ActiveTheme {
  id: string;
  dark: boolean;
  roles: ThemeRoles;
}

/** Appearance payload from get_appearance, launcher://shown and tray://menu; mirrors AppearanceView. */
export interface Appearance {
  scheme: Scheme;
  darkTheme: string;
  lightTheme: string;
  /** Null when Rust could not resolve a theme; tokens.css Flick Default values apply then. */
  active: ActiveTheme | null;
}

/** One entry of list_themes; mirrors ThemeView in Rust. */
export interface ThemeView {
  id: string;
  name: string;
  source: "builtin" | "port" | "imported";
  dark: boolean;
  roles: ThemeRoles;
  ratios: {
    text: number;
    muted: number;
    accentText: number;
    onAccent: number;
    selText: number;
    mutedBg: number;
    mutedSel: number;
  };
  /** Roles Rust lifted to reach contrast. */
  adjusted: string[];
  upstream: string | null;
}

/** Signature of the last theme written per element, so an unchanged theme costs no style writes. */
const applied = new WeakMap<HTMLElement, string>();

const prefersDark = () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;

/**
 * Writes the role colours as inline `--l-*` properties plus `data-scheme`. Skipped when the role
 * signature (hex values + scheme) equals the last one applied to `root`. `null` removes the inline
 * properties so the tokens.css fallback applies, with the scheme from prefers-color-scheme.
 * Called on show and on settings changes only, never on the keystroke path.
 */
export function applyTheme(root: HTMLElement, active: ActiveTheme | null) {
  const scheme = (active ? active.dark : prefersDark()) ? "dark" : "light";
  const signature = active ? `${ROLES.map((r) => active.roles[r]).join()}|${scheme}` : scheme;
  if (applied.get(root) === signature) return;
  applied.set(root, signature);
  for (const role of ROLES) {
    if (active) root.style.setProperty(ROLE_VARS[role], active.roles[role]);
    else root.style.removeProperty(ROLE_VARS[role]);
  }
  root.dataset.scheme = scheme;
}

// Legacy exports below: removed in U1b2 (the tour still uses them) and U1b3 (tokens.contrast.test.ts).
export const THEMES = ["glass", "frost", "midnight", "aurora", "ember", "solid"] as const;
export type ThemeId = (typeof THEMES)[number];

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

interface LegacyAppearance {
  theme: ThemeId;
  glass: number;
  scheme: Scheme;
}

/** The dark/light scheme a legacy theme renders in. */
export function resolveScheme(a: LegacyAppearance, systemDark: boolean): "dark" | "light" {
  const forced = FORCED_SCHEME[a.theme];
  if (forced) return forced;
  return a.scheme === "system" ? (systemDark ? "dark" : "light") : a.scheme;
}
