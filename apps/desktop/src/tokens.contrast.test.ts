import { describe, expect, it } from "vitest";
import css from "./tokens.css?raw";

// The CSS keeps only the fallbacks; named themes come from Rust, whose tests assert their contrast.
// dark: `:root`. light-attr: `[data-scheme="light"]`. light-media: the prefers-color-scheme fallback.
const MODES = ["dark", "light-attr", "light-media"] as const;
type Mode = (typeof MODES)[number];
type Rgb = readonly [number, number, number];
interface Rule {
  selectors: string[];
  decls: Map<string, string>;
  /** True for rules inside `@media (prefers-color-scheme: light)`. */
  media: boolean;
}

const LIGHT_MEDIA = "@media (prefers-color-scheme: light)";

/** Reads the rules of tokens.css in source order. Comments and at-rules other than the light media query are skipped. */
function parseRules(src: string, media = false): Rule[] {
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
    const prelude = text.slice(i, open).replace(/\s+/g, " ").trim();
    const body = text.slice(open + 1, j - 1);
    i = j;
    if (prelude === LIGHT_MEDIA) {
      rules.push(...parseRules(body, true));
      continue;
    }
    if (prelude.startsWith("@")) continue;
    const decls = new Map<string, string>();
    for (const part of body.split(";")) {
      const colon = part.indexOf(":");
      if (colon < 0) continue;
      decls.set(part.slice(0, colon).trim(), part.slice(colon + 1).trim());
    }
    const selectors = prelude.split(",").map((s) => s.replace(/\s+/g, " ").trim());
    rules.push({ selectors, decls, media });
  }
  return rules;
}

const RULES = parseRules(css);

/** True or false when the selector is in the supported grammar, null when it is not. */
function applies(selector: string, mode: Mode): boolean | null {
  if (selector === ":root" || selector === "[data-theme]" || selector === "[data-scheme]") {
    return true;
  }
  if (selector === '[data-scheme="light"]') return mode === "light-attr";
  if (selector === ":root:not([data-scheme])") return mode === "light-media";
  return null;
}

/** Selectors that are not part of the role cascade. */
const ALLOWED_UNSUPPORTED = [".tile-letter"];

/** All selectors have equal specificity, so source order is the cascade. */
function declared(name: string, mode: Mode): string | undefined {
  let value: string | undefined;
  for (const rule of RULES) {
    if (rule.media && mode !== "light-media") continue;
    if (!rule.selectors.some((s) => applies(s, mode) === true)) continue;
    const d = rule.decls.get(name);
    if (d !== undefined) value = d;
  }
  return value;
}

function resolve(name: string, mode: Mode, depth = 0): string {
  const value = declared(name, mode);
  if (value === undefined) throw new Error(`${name} is not defined for ${mode}`);
  const ref = /^var\((--[a-z0-9-]+)\)$/.exec(value);
  if (ref?.[1]) {
    if (depth >= 10) throw new Error(`${name}: var() chain deeper than 10`);
    return resolve(ref[1], mode, depth + 1);
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

// Text roles need 4.5:1 (WCAG AA). Controls and the accent as a fill edge need 3:1 (WCAG 1.4.11).
// --l-line and --l-hairline are decorative separators and are exempt.
const TEXT_ROLES = ["text", "muted", "muted-strong", "accent-text", "ok-text", "err", "warn"];
const EDGE_ROLES = ["control-edge", "focus", "accent"];
// The opaque surfaces text and controls sit on.
const SURFACES = ["panel", "bg", "sel"];

describe("tokens.css contrast", () => {
  it.each(MODES)("%s fallback meets AA on panel, bg and sel", (mode) => {
    const color = (role: string) => toRgb(resolve(`--l-${role}`, mode));
    const failures: string[] = [];
    for (const surface of SURFACES) {
      const bg = color(surface);
      const check = (role: string, min: number) => {
        const got = ratio(color(role), bg);
        if (got < min) failures.push(`${mode} ${role} on ${surface} ${got.toFixed(2)} < ${min}`);
      };
      for (const role of TEXT_ROLES) check(role, 4.5);
      for (const role of EDGE_ROLES) check(role, 3);
    }
    const onAccent = ratio(color("on-accent"), color("accent"));
    if (onAccent < 4.5) failures.push(`${mode} on-accent on accent ${onAccent.toFixed(2)} < 4.5`);
    expect(failures).toEqual([]);
  });

  it("only known selectors take part in the role cascade", () => {
    const unknown = RULES.flatMap((r) => r.selectors)
      .filter((s) => applies(s, "dark") === null)
      .filter((s) => !ALLOWED_UNSUPPORTED.includes(s));
    expect(unknown).toEqual([]);
  });

  it("defines every role in dark and in light", () => {
    const roles = [
      "bg panel surface line text muted muted-strong accent accent-text on-accent sel",
      "ok ok-text err warn scrim focus control-edge hover hairline rim-top rim-low spec",
      "sheen raise raise-hover key-bottom warn-wash droplet",
      "shadow-raised shadow-overlay shadow-window",
    ].flatMap((line) => line.split(" "));
    for (const mode of MODES) {
      const missing = roles.filter((r) => declared(`--l-${r}`, mode) === undefined);
      expect(missing).toEqual([]);
    }
  });

  it("derives six chrome roles from the theme roles in the shared alias block", () => {
    const expected: Record<string, string> = {
      "--l-muted-strong": "var(--l-muted)",
      "--l-ok-text": "var(--l-ok)",
      "--l-scrim": "color-mix(in oklab, var(--l-bg) 70%, transparent)",
      "--l-focus": "var(--l-accent-text)",
      "--l-control-edge": "var(--l-muted)",
      "--l-hover": "var(--l-surface)",
    };
    const shared = [":root", "[data-theme]", "[data-scheme]"];
    for (const [name, value] of Object.entries(expected)) {
      const owners = RULES.filter((r) => r.decls.has(name));
      expect(owners.length, `${name} is declared once`).toBe(1);
      const owner = owners[0];
      expect(owner?.selectors, name).toEqual(shared);
      expect(owner?.media, name).toBe(false);
      expect(owner?.decls.get(name), name).toBe(value);
    }
  });

  it("is flat: square radii, mono chrome font, prose font, 13px body, no inset shadows", () => {
    const get = (n: string) => declared(n, "dark");
    for (const r of ["key", "control", "card"]) expect(get(`--l-r-${r}`)).toBe("2px");
    expect(get("--l-ui")).toMatch(/^"IBM Plex Mono"/);
    expect(get("--l-prose")).toMatch(/^"IBM Plex Sans"/);
    expect(get("--l-fs-body")).toBe("13px");
    for (const s of ["raised", "overlay", "window"]) {
      expect(get(`--l-shadow-${s}`)).not.toMatch(/inset|gradient/);
    }
    expect(get("--l-hairline")).toBe("var(--l-line)");
  });
});
