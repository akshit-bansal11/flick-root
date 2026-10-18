import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Markdown } from "./Markdown";

let root: Root | undefined;
let box: HTMLElement;
const open = vi.fn();

function mount(text: string) {
  box = document.body.appendChild(document.createElement("div"));
  root = createRoot(box);
  act(() => root?.render(<Markdown text={text} onOpenLink={open} />));
}

afterEach(() => {
  act(() => root?.unmount());
  box.remove();
  open.mockClear();
});

describe("Markdown", () => {
  it.each([
    "<script>alert(1)</script>",
    "<img src=x onerror=alert(1)>",
    "[x](javascript:alert(1))",
    "[x](data:text/html,<b>hi</b>)",
    "![a](https://evil/x.png)",
    '<a href="https://evil">z</a>',
  ])("neutralises %s", (src) => {
    mount(src);
    expect(box.querySelector("script, img, a[href], [onerror]")).toBeNull();
    expect(box.textContent?.length).toBeGreaterThan(0);
  });

  it("shows raw HTML literally", () => {
    mount("<script>alert(1)</script> and [x](javascript:alert(1))");
    expect(box.textContent).toContain("<script>alert(1)</script>");
    expect(box.textContent).toContain("x");
  });

  it("renders expected elements", () => {
    mount("# H\n\n**b** *i* `c`\n\n- one\n- two\n\n```\ncode\n```");
    for (const s of ["h1", "strong", "em", "code", "ul li", "pre"])
      expect(box.querySelector(s)).not.toBeNull();
  });

  it("tolerates unterminated input", () => {
    expect(() => mount("```py\nx = **1")).not.toThrow();
    expect(box.querySelector("pre")?.textContent).toContain("x = **1");
  });

  it("click calls onOpenLink without navigating", () => {
    mount("[go](https://example.com/p)");
    const b = box.querySelector("button");
    expect(b?.getAttribute("title")).toBe("example.com");
    act(() => {
      b?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(open).toHaveBeenCalledWith("https://example.com/p");
    expect(box.querySelector("a")).toBeNull();
  });
});
