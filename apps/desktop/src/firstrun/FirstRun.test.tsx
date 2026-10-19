import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../settings/api";
import { Detect, NONE_FOUND } from "./Detect";
import { FirstRun } from "./FirstRun";
import { Hotkey } from "./Hotkey";
import { comboFromEvent, validateCombo } from "./hotkeyRules";

vi.mock("../tour/Tour", () => ({
  Tour: ({ onDone }: { onDone: (c: boolean) => void }) => (
    <button type="button" data-testid="tour" onClick={() => onDone(true)}>
      tour
    </button>
  ),
}));
vi.mock("../settings/api", () => ({
  getSettings: vi.fn(() => Promise.resolve({ hotkeyExplain: "Ctrl+Shift+E" })),
  setGeneral: vi.fn(),
  probeProviders: vi.fn(),
  setProvider: vi.fn(() => Promise.resolve({})),
}));

const core = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => core);

let root: Root | null = null;
let host: HTMLElement;
beforeEach(() => {
  host = document.createElement("div");
  document.body.append(host);
});
afterEach(() => {
  act(() => root?.unmount());
  host.remove();
  vi.clearAllMocks();
});
const mount = async (el: ReactElement) => {
  root = createRoot(host);
  await act(async () => root?.render(el));
};
const click = (el: Element | null | undefined) => act(async () => (el as HTMLElement).click());
const key = (init: KeyboardEventInit) =>
  act(async () => {
    host
      .querySelector("button")
      ?.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
  });
const probe = (id: string, reachable: boolean) => ({
  id,
  reachable,
  models: ["m1"],
  ms: 5,
  error: null,
});

describe("hotkeyRules", () => {
  it.each([
    ["Space", "Add Ctrl, Alt, Shift or Win."],
    ["Super+L", "Windows keeps this shortcut."],
    ["Ctrl+Shift+E", "This shortcut is used for Explain this."],
    ["Alt+Space", null],
  ])("%s", (combo, msg) => {
    expect(validateCombo(combo, "Ctrl+Shift+E")).toBe(msg);
  });

  it("builds a combo and ignores bare modifiers", () => {
    const e = {
      key: " ",
      code: "Space",
      ctrlKey: false,
      altKey: true,
      shiftKey: false,
      metaKey: false,
    };
    expect(comboFromEvent(e)).toBe("Alt+Space");
    expect(comboFromEvent({ ...e, key: "Alt", code: "AltLeft" })).toBeNull();
  });
});

describe("Hotkey", () => {
  it("shows the conflict message and stays in capture mode", async () => {
    vi.mocked(api.setGeneral).mockRejectedValue({ code: "hotkey_conflict", message: "x" });
    await mount(
      <Hotkey initial="Alt+Space" initialConflict={false} explainHotkey="" onValid={() => {}} />,
    );
    await click(host.querySelector("button"));
    await key({ key: "k", code: "KeyK", ctrlKey: true });
    expect(host.textContent).toContain(
      "Ctrl+K is already used by another app. PowerToys Run is a common one. Pick another shortcut.",
    );
    expect(host.textContent).toContain("Press a shortcut");
  });
});

describe("Detect", () => {
  it("lists servers with switches off and enables nothing until clicked", async () => {
    vi.mocked(api.probeProviders).mockResolvedValue([
      probe("ollama", true),
      probe("lmstudio", false),
    ]);
    await mount(<Detect />);
    const sw = host.querySelectorAll('[role="switch"]');
    expect(sw).toHaveLength(1);
    expect(sw[0]?.getAttribute("aria-checked")).toBe("false");
    expect(api.setProvider).not.toHaveBeenCalled();
    await click(sw[0]);
    expect(api.setProvider).toHaveBeenCalledWith("ollama", { enabled: true });
  });

  it("shows the none-found message", async () => {
    vi.mocked(api.probeProviders).mockResolvedValue([probe("ollama", false)]);
    await mount(<Detect />);
    expect(host.textContent).toContain(NONE_FOUND);
  });
});

describe("FirstRun", () => {
  const setup = async () => {
    core.invoke.mockImplementation((cmd: string) =>
      Promise.resolve(
        cmd === "get_first_run"
          ? { done: false, hotkey: "Alt+Space", hotkeyRegistered: true }
          : undefined,
      ),
    );
    vi.mocked(api.probeProviders).mockResolvedValue([]);
    await mount(<FirstRun />);
  };
  const byText = (t: string) =>
    [...host.querySelectorAll("button")].find((b) => b.textContent === t);

  it("yes: welcome, tour, hotkey, models, finish", async () => {
    await setup();
    expect(host.textContent).toContain("Take a 2-minute interactive tour?");
    await click(byText("Take the tour"));
    await click(host.querySelector('[data-testid="tour"]'));
    expect(host.textContent).toContain("Step 1 of 2");
    await click(host.querySelector(".fr-primary"));
    expect(host.textContent).toContain("Step 2 of 2");
    await click(host.querySelector(".fr-primary"));
    expect(core.invoke).toHaveBeenCalledWith("complete_first_run");
  });

  it("no: note, then hotkey step", async () => {
    await setup();
    await click(byText("Not now"));
    expect(host.textContent).toContain("Settings > General > Take the tour");
    await click(byText("Continue"));
    expect(host.textContent).toContain("Step 1 of 2");
  });
});
