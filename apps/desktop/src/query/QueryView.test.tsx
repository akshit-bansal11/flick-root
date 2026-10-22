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
  it("shows a live chip for the selected thinking mode", () => {
    const modes = [
      { id: "a/m1", providerId: "a", providerLabel: "Ollama", model: "m1", local: true },
      { id: "b/m2", providerId: "b", providerLabel: "OpenAI", model: "m2", local: false },
    ];
    const c = render({ modes, selectedMode: "b/m2" });
    const chip = c.querySelector(".query-chip");
    expect(chip?.textContent).toBe("OpenAI · m2");
    expect(chip?.getAttribute("aria-live")).toBe("polite");
    act(() => root?.render(<QueryView {...base} modes={modes} selectedMode="a/m1" />));
    expect(c.querySelector(".query-chip")?.textContent).toBe("Ollama · m1 · local");
  });

  it("keeps the AI answer marker while streaming and after the answer is done", () => {
    const ai = { provider: "Local", model: "llama3.2" };
    const c = render({ ...ai, streaming: true });
    expect(c.querySelector(".ai-mark")?.textContent).toBe("AI answer · Local · llama3.2");
    act(() => root?.render(<QueryView {...base} {...ai} streaming text="Hi" />));
    expect(c.querySelector(".ai-mark")?.textContent).toBe("AI answer · Local · llama3.2");
    act(() => root?.render(<QueryView {...base} {...ai} text="Hi there" />));
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

  it("empty state renders copy and button opens settings", () => {
    const open = vi.fn();
    const c = render({ noProvider: true, onOpenSettings: open });
    expect(c.textContent).toContain("Connect a model to ask questions");
    act(() => c.querySelector("button")?.click());
    expect(open).toHaveBeenCalledOnce();
  });

  it("Fast/Think pill toggles on click", () => {
    const toggle = vi.fn();
    const c = render({ onToggleEffort: toggle });
    const pill = c.querySelector(".query-pill") as HTMLButtonElement;
    expect(pill.textContent).toBe("Fast");
    act(() => pill.click());
    expect(toggle).toHaveBeenCalledOnce();
    act(() => root?.render(<QueryView {...base} effort="think" />));
    expect(c.querySelector(".query-pill")?.textContent).toBe("Think");
  });
});
