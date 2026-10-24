import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryView } from "./QueryView";

let root: Root | undefined;
const base = {
  question: "why",
  provider: "Ollama",
  model: null,
  effort: "fast" as const,
  text: "",
  error: null,
  noProvider: false,
  onToggleEffort: () => {},
  onOpenSettings: () => {},
  onOpenLink: () => {},
};
const modes = [
  { id: "a/m1", providerId: "a", providerLabel: "Ollama", model: "m1", local: true },
  { id: "b/m2", providerId: "b", providerLabel: "OpenAI", model: "m2", local: false },
];
const render = (props: Partial<Parameters<typeof QueryView>[0]>) => {
  const c = document.body.appendChild(document.createElement("div"));
  root = createRoot(c);
  act(() => root?.render(<QueryView {...base} {...props} />));
  return c;
};

beforeEach(() => Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true }));
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
});

describe("QueryView", () => {
  it("lists model rows and marks the selected mode", () => {
    const c = render({ modes, selectedMode: "b/m2" });
    const rows = [...c.querySelectorAll('[role="listbox"][aria-label="Models"] [role="option"]')];
    expect(rows.map((r) => r.querySelector(".row-title")?.textContent)).toEqual([
      "Ollama/m1",
      "OpenAI/m2",
    ]);
    expect(rows.map((r) => r.getAttribute("aria-selected"))).toEqual(["false", "true"]);
    expect(rows[1]?.querySelector(".row-mark")?.textContent).toBe("›");
    act(() => root?.render(<QueryView {...base} modes={modes} selectedMode="a/m1" />));
    const after = c.querySelectorAll('[role="option"]');
    expect(after[0]?.getAttribute("aria-selected")).toBe("true");
    expect(after[1]?.getAttribute("aria-selected")).toBe("false");
  });

  it("tags only local modes", () => {
    const c = render({ modes });
    const tags = [...c.querySelectorAll('[role="option"]')].map(
      (r) => r.querySelector(".tag")?.textContent ?? null,
    );
    expect(tags).toEqual(["local", null]);
  });

  it("keeps the AI answer marker while streaming and after the answer is done", () => {
    const ai = { provider: "Local", model: "llama3.2" };
    const c = render({ ...ai, streaming: true });
    const marker = "AI answer · Local · llama3.2 · fast";
    expect(c.querySelector(".ai-mark")?.textContent).toBe(marker);
    act(() => root?.render(<QueryView {...base} {...ai} streaming text="Hi" />));
    expect(c.querySelector(".ai-mark")?.textContent).toBe(marker);
    act(() => root?.render(<QueryView {...base} {...ai} text="Hi there" />));
    expect(c.querySelector(".ai-mark")?.textContent).toBe(marker);
    act(() => root?.render(<QueryView {...base} {...ai} effort="think" text="Hi there" />));
    expect(c.querySelector(".ai-mark")?.textContent).toBe("AI answer · Local · llama3.2 · think");
  });

  it("leaves effort out of the marker when the provider has no reasoning control", () => {
    const c = render({ provider: "Local", model: "llama3.2", reasoning: false, text: "Hi" });
    expect(c.querySelector(".ai-mark")?.textContent).toBe("AI answer · Local · llama3.2");
  });

  it("shows no AI marker without text or streaming", () => {
    expect(render({ provider: "Local", model: "llama3.2" }).querySelector(".ai-mark")).toBeNull();
  });

  it("appends streamed deltas in order", () => {
    const c = render({ text: "Hel" });
    act(() => root?.render(<QueryView {...base} text="Hello, world" />));
    expect(c.querySelector("[data-answer]")?.textContent).toBe("Hello, world");
  });

  it("shows thinking marker before first text only", () => {
    expect(render({ thinking: true }).textContent).toContain("thinking…");
    act(() => root?.render(<QueryView {...base} thinking text="a" />));
    expect(document.body.textContent).not.toContain("thinking…");
  });

  it("renders no models list without modes", () => {
    expect(render({}).querySelector('[role="listbox"]')).toBeNull();
  });

  it("shows the error in an alert", () => {
    const c = render({ error: "Provider unreachable" });
    expect(c.querySelector('[role="alert"]')?.textContent).toBe("Provider unreachable");
  });

  it("empty state renders copy and button opens settings", () => {
    const open = vi.fn();
    const c = render({ noProvider: true, onOpenSettings: open });
    expect(c.textContent).toContain("Connect a model to ask questions");
    act(() => c.querySelector("button")?.click());
    expect(open).toHaveBeenCalledOnce();
  });

  it("effort row reflects the effort and toggles on click", () => {
    const toggle = vi.fn();
    const c = render({ onToggleEffort: toggle });
    const btn = () => c.querySelector("button.row.effort");
    expect(btn()?.getAttribute("aria-pressed")).toBe("false");
    expect(btn()?.textContent).toBe("fastctrl+t");
    act(() => btn()?.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(toggle).toHaveBeenCalledOnce();
    act(() => root?.render(<QueryView {...base} effort="think" />));
    expect(btn()?.getAttribute("aria-pressed")).toBe("true");
    expect(btn()?.textContent).toBe("thinkctrl+t");
  });

  it("hides the effort row when reasoning is false", () => {
    expect(render({ reasoning: false }).querySelector(".effort")).toBeNull();
  });
});
