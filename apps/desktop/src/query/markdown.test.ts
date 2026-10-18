import { describe, expect, it } from "vitest";
import { isSafeUrl, parseInline, parseMarkdown } from "./mdast";

describe("parseMarkdown", () => {
  it("parses blocks", () => {
    const b = parseMarkdown(
      "# T\n\npara **b** *i* `c`\n\n- a\n  1. x\n- b\n\n> q\n\n---\n\n```rs\nfn x() {}\n```",
    );
    expect(b.map((x) => x.t)).toEqual(["h", "p", "list", "quote", "hr", "code"]);
    expect(b[5]).toEqual({ t: "code", lang: "rs", text: "fn x() {}" });
  });

  it("open fence runs to the end and half-typed emphasis is literal", () => {
    expect(parseMarkdown("```js\nlet a")[0]).toEqual({ t: "code", lang: "js", text: "let a" });
    expect(parseInline("hi **bold")).toEqual([{ t: "text", v: "hi **bold" }]);
  });

  it("only http(s) links survive", () => {
    expect(isSafeUrl("javascript:alert(1)")).toBe(false);
    expect(isSafeUrl("data:text/html,x")).toBe(false);
    expect(isSafeUrl("/rel")).toBe(false);
    expect(isSafeUrl("https://a.b/c")).toBe(true);
    expect(parseInline("see https://a.b/c.")[1]).toMatchObject({ t: "link", url: "https://a.b/c" });
  });

  it("is fast on 200 KB, including pathological input", () => {
    for (const unit of ["hello **world** [a](https://x.y) `c`\n", "[", "*a ", "> "]) {
      const t0 = performance.now();
      parseMarkdown(unit.repeat(Math.ceil(200_000 / unit.length)));
      expect(performance.now() - t0).toBeLessThan(50);
    }
  });
});
