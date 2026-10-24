import { describe, expect, it } from "vitest";
import { GLASS_DEFAULT, GLASS_MAX, GLASS_MIN } from "./theme";
import css from "./tokens.css?raw";

type Scheme = "dark" | "light";

// U1b3 deletes the CSS theme blocks and this list. Ids with a `[data-theme=...]` block left in tokens.css,
// and the scheme each one forces (none: it follows the scheme).
const THEMES = ["glass", "frost", "midnight", "aurora", "ember", "solid"] as const;
type ThemeId = (typeof THEMES)[number];
const FORCED_SCHEME: Partial<Record<ThemeId, Scheme>> = {
  frost: "light",
  midnight: "dark",
  aurora: "dark",
  ember: "dark",
};
type Rgb = readonly [number, number, number];
interface Rule {
  selectors: string[];
  decls: Map<string, string>;
}

/** Reads the flat rules of tokens.css in source order. Comments and at-rules (@font-face, @media) are skipped. */
function parseRules(src: string): Rule[] {
  const text = src.replace(/\/\*[\s\S]*?\*\//g, "");
  const rules: Rule[] = [];
  let i = 0;
  while (i < text.length) {
    const open = text.indexOf("{", i);
    if (open < 0) break;
    let depth = 1;
    let j = open + 1;
    while (j < text.length && depth > 0) {
      const ch = text[j];
      if (ch === "{") depth++;
      else if (ch === "}") depth--;
      j++;
    }
    const prelude = text.slice(i, open).trim();
    const body = text.slice(open + 1, j - 1);
    i = j;
    if (prelude.startsWith("@")) continue;
    const decls = new Map<string, string>();
    for (const part of body.split(";")) {
      const colon = part.indexOf(":");
      if (colon < 0) continue;
      decls.set(part.slice(0, colon).trim(), part.slice(colon + 1).trim());
    }
    const selectors = prelude.split(",").map((s) => s.replace(/\s+/g, " ").trim());
    rules.push({ selectors, decls });
  }
  return rules;
}

const RULES = parseRules(css);

/** True or false when the selector is in the supported grammar, null when it is not. */
function applies(selector: string, theme: ThemeId, scheme: Scheme): boolean | null {
  if (selector === ":root" || selector === "[data-theme]" || selector === "[data-scheme]") {
    return true;
  }
  const m = /^\[data-(theme|scheme)="([a-z]+)"\]$/.exec(selector);
  if (!m) return null;
  return m[1] === "theme" ? m[2] === theme : m[2] === scheme;
}

/** Selectors that are not part of the role cascade. */
const ALLOWED_UNSUPPORTED = [".tile-letter"];

/** All selectors have equal specificity, so source order is the cascade. */
function declared(name: string, theme: ThemeId, scheme: Scheme): string | undefined {
  let value: string | undefined;
  for (const rule of RULES) {
    if (!rule.selectors.some((s) => applies(s, theme, scheme) === true)) continue;
    const d = rule.decls.get(name);
    if (d !== undefined) value = d;
  }
  return value;
}

function resolve(name: string, theme: ThemeId, scheme: Scheme, depth = 0): string {
  const value = declared(name, theme, scheme);
  if (value === undefined) throw new Error(`${name} is not defined for ${theme}/${scheme}`);
  const ref = /^var\((--[a-z0-9-]+)\)$/.exec(value);
  if (ref?.[1]) {
    if (depth >= 10) throw new Error(`${name}: var() chain deeper than 10`);
    return resolve(ref[1], theme, scheme, depth + 1);
  }
  return value;
}

function toRgb(hex: string): Rgb {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex);
  const h = m?.[1];
  if (!h) throw new Error(`not a #rgb/#rrggbb colour: ${hex}`);
  const f = h.length === 3 ? [...h].map((c) => c + c).join("") : h;
  return [
    Number.parseInt(f.slice(0, 2), 16),
    Number.parseInt(f.slice(2, 4), 16),
    Number.parseInt(f.slice(4, 6), 16),
  ];
}

function luminance([r, g, b]: Rgb): number {
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function ratio(a: Rgb, b: Rgb): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** The panel tint over a backdrop: alpha * panel + (1 - alpha) * backdrop per sRGB channel, no rounding. */
function blend(panel: Rgb, backdrop: Rgb, alpha: number): Rgb {
  const mix = (p: number, k: number) => alpha * p + (1 - alpha) * k;
  return [mix(panel[0], backdrop[0]), mix(panel[1], backdrop[1]), mix(panel[2], backdrop[2])];
}

/** Smallest glass percent at which fg reaches `min` against the tint (bisection); above 100 when never. */
function requiredAlpha(fg: Rgb, panel: Rgb, backdrop: Rgb, min: number): number {
  const ok = (a: number) => ratio(fg, blend(panel, backdrop, a)) >= min;
  if (!ok(1)) return 101;
  if (ok(0)) return 0;
  let lo = 0;
  let hi = 1;
  for (let n = 0; n < 30; n++) {
    const mid = (lo + hi) / 2;
    if (ok(mid)) hi = mid;
    else lo = mid;
  }
  return Math.ceil(hi * 100);
}

const WHITE: Rgb = [255, 255, 255];
const BLACK: Rgb = [0, 0, 0];

// Text roles need 4.5:1 (WCAG AA). Controls and the accent as a fill edge need 3:1 (WCAG 1.4.11).
// --l-line and --l-hairline are decorative separators and are exempt.
const TEXT_ROLES = ["text", "muted", "muted-strong", "accent-text", "ok-text", "err", "warn"];
const EDGE_ROLES = ["control-edge", "focus", "accent"];

interface Case {
  label: string;
  alpha: number;
  backdrop: Rgb;
}

function casesFor(theme: ThemeId): Case[] {
  const opaque: Case = { label: "opaque", alpha: 1, backdrop: WHITE };
  if (theme === "solid") return [opaque];
  const alpha = GLASS_MIN / 100;
  return [
    opaque,
    { label: "glass on white", alpha, backdrop: WHITE },
    { label: "glass on black", alpha, backdrop: BLACK },
  ];
}

const COMBOS = THEMES.flatMap((theme) => {
  const forced = FORCED_SCHEME[theme];
  const schemes: Scheme[] = forced ? [forced] : ["dark", "light"];
  return schemes.map((scheme) => ({ theme, scheme }));
});

describe("tokens.css contrast", () => {
  it.each(COMBOS)("$theme/$scheme meets AA over the worst backdrops", ({ theme, scheme }) => {
    const color = (role: string) => toRgb(resolve(`--l-${role}`, theme, scheme));
    const panel = color("panel");
    const failures: string[] = [];
    for (const c of casesFor(theme)) {
      const tint = blend(panel, c.backdrop, c.alpha);
      const check = (role: string, min: number) => {
        const fg = color(role);
        const got = ratio(fg, tint);
        if (got >= min) return;
        const need = requiredAlpha(fg, panel, c.backdrop, min);
        const hint = c.alpha < 1 ? `; needs glass >= ${need}%` : "";
        failures.push(`${theme}/${scheme} ${c.label} ${role} ${got.toFixed(2)} < ${min}${hint}`);
      };
      for (const role of TEXT_ROLES) check(role, 4.5);
      for (const role of EDGE_ROLES) check(role, 3);
    }
    const onAccent = ratio(color("on-accent"), color("accent"));
    if (onAccent < 4.5) {
      failures.push(`${theme}/${scheme} on-accent on accent ${onAccent.toFixed(2)} < 4.5`);
    }
    expect(failures).toEqual([]);
  });

  it("only known selectors take part in the role cascade", () => {
    const unknown = RULES.flatMap((r) => r.selectors)
      .filter((s) => applies(s, "glass", "dark") === null)
      .filter((s) => !ALLOWED_UNSUPPORTED.includes(s));
    expect(unknown).toEqual([]);
  });

  it("defines every role in dark and in light", () => {
    const roles = [
      "bg panel surface line text muted muted-strong accent accent-text on-accent sel",
      "ok ok-text err warn scrim focus control-edge hover hairline rim-top rim-low spec",
      "sheen raise raise-hover key-bottom warn-wash droplet mica-tint",
      "shadow-raised shadow-overlay shadow-window",
    ].flatMap((line) => line.split(" "));
    for (const scheme of ["dark", "light"] as const) {
      const missing = roles.filter((r) => declared(`--l-${r}`, "glass", scheme) === undefined);
      expect(missing).toEqual([]);
    }
  });

  it("is flat: square radii, mono chrome font, prose font, 13px body, no inset shadows", () => {
    const get = (n: string) => declared(n, "glass", "dark");
    for (const r of ["key", "control", "card"]) expect(get(`--l-r-${r}`)).toBe("2px");
    expect(get("--l-ui")).toMatch(/^"IBM Plex Mono"/);
    expect(get("--l-prose")).toMatch(/^"IBM Plex Sans"/);
    expect(get("--l-fs-body")).toBe("13px");
    for (const s of ["raised", "overlay", "window"]) {
      expect(get(`--l-shadow-${s}`)).not.toMatch(/inset|gradient/);
    }
    expect(get("--l-hairline")).toBe("var(--l-line)");
  });

  it("keeps the glass range ordered", () => {
    expect(GLASS_MIN).toBeLessThanOrEqual(GLASS_DEFAULT);
    expect(GLASS_DEFAULT).toBeLessThanOrEqual(GLASS_MAX);
  });
});
