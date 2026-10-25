import { describe, expect, it } from "vitest";

const files = import.meta.glob<string>(["./**/*.{css,tsx}", "!./**/*.test.tsx"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");
const all = Object.entries(files).map(([path, text]) => ({ path, text: strip(text) }));
const tokens = all.filter((f) => f.path === "./tokens.css");
const others = all.filter((f) => f.path !== "./tokens.css");
const css = others.filter((f) => f.path.endsWith(".css"));
const tsx = others.filter((f) => f.path.endsWith(".tsx"));

const LEGACY = [
  "ink",
  "r",
  "r2",
  "r-tile",
  "r-panel",
  "fs",
  "lh",
  "fs-sub",
  "fs-meta",
  "fs-action",
  "fs-toast",
  "fs-group",
  "fs-math",
  "ease-spring",
  "ease-in-out",
  "shadow",
  "shadow-panel",
  "shadow-settings",
  "highlight",
  "glass-border",
  "glass-sheen",
  "glass-inner",
];
const legacyRe = new RegExp(`--l-(${LEGACY.join("|")})(?![\\w-])`);

/** Paths of files whose text matches. */
const hits = (list: { path: string; text: string }[], re: RegExp) =>
  list.filter((f) => re.test(f.text)).map((f) => f.path);

describe("token hygiene", () => {
  it("reads the CSS files", () => {
    expect(tokens).toHaveLength(1);
    expect(css.length).toBeGreaterThan(5);
  });

  it("uses no legacy alias name anywhere", () => {
    expect(hits(all, legacyRe)).toEqual([]);
  });

  it("references only tokens that are declared", () => {
    const declared = new Set(
      all.flatMap((f) => [...f.text.matchAll(/(--l-[\w-]+)\s*:/g)].map((m) => m[1])),
    );
    const missing = all.flatMap((f) =>
      [...f.text.matchAll(/var\((--l-[\w-]+)/g)]
        .map((m) => m[1] ?? "")
        .filter((n) => !declared.has(n))
        .map((n) => `${f.path} ${n}`),
    );
    expect(missing).toEqual([]);
  });

  it("keeps colour literals in tokens.css", () => {
    expect(hits(css, /:[^;{}]*#[0-9a-f]{3,8}\b/i)).toEqual([]);
    expect(hits(css, /\b(?:rgba?|hsla?)\(/i)).toEqual([]);
    expect(hits(css, /\b(?:oklch|oklab)\(/i)).toEqual([]);
    expect(hits(css, /(?<![\w-])color\(/i)).toEqual([]);
  });

  it("sets font sizes through tokens", () => {
    expect(hits(css, /font-size:[^;]*\d\s*(?:px|r?em|pt)\b/i)).toEqual([]);
    expect(hits(css, /font:[^;]*\d+(?:\.\d+)?(?:px|r?em|pt)\b/i)).toEqual([]);
    expect(hits(tsx, /fontSize/)).toEqual([]);
  });

  it("uses sentence-case labels", () => {
    expect(hits(css, /text-transform:\s*uppercase/)).toEqual([]);
  });

  it("reads raw palette values only inside tokens.css", () => {
    expect(hits(others, /--l-raw-/)).toEqual([]);
  });

  it("styles flat: no gradient, blur, mask compositing or inset shadow", () => {
    expect(hits(all, /gradient\(/)).toEqual([]);
    expect(hits(all, /backdrop-filter/)).toEqual([]);
    expect(hits(all, /mask-composite/)).toEqual([]);
    expect(hits(all, /box-shadow:[^;]*\binset\b/)).toEqual([]);
  });

  it("keeps no selection-pill, liquid filter or glass attribute leftovers", () => {
    expect(hits(all, /\bL[e]ns\b|\bLiquidFilter\b/)).toEqual([]);
    expect(hits(all, /data-gl[a]ss/)).toEqual([]);
  });
});
