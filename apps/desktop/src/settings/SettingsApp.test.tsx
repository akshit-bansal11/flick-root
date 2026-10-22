import { fireEvent, screen } from "@testing-library/react";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../ipc";
import * as api from "./api";
import type { PageId } from "./Nav";
import { resolveView, SettingsApp } from "./SettingsApp";
import type { SettingsView } from "./types";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("../firstrun/FirstRun", () => ({ FirstRun: () => null }));
vi.mock("../ipc", () => ({ closeWindow: vi.fn(() => Promise.resolve()) }));
vi.mock("./api", () => ({
  getSettings: vi.fn(),
  setGeneral: vi.fn(),
  setRoots: vi.fn(),
  clearHistory: vi.fn(() => Promise.resolve()),
  reindex: vi.fn(() => Promise.resolve()),
  listFeatures: vi.fn(),
  onFeaturesChanged: vi.fn(() => Promise.resolve(() => {})),
  getAppearance: vi.fn(() => Promise.resolve({ theme: "glass", glass: 62, scheme: "system" })),
  onSettingsChanged: vi.fn(() => Promise.resolve(() => {})),
}));

const settings: SettingsView = {
  hotkeyLauncher: "Alt+Space",
  hotkeyExplain: "Alt+Shift+Space",
  launchAtStartup: false,
  findEnabled: true,
  includeDrives: true,
  scanPrograms: true,
  queryEnabled: false,
  roots: [],
  extraRoots: [],
  programSkip: [],
  drives: [],
  programsCount: 0,
  programsCapped: false,
};

let root: Root | undefined;

function Harness() {
  const [page, setPage] = useState<PageId>("general");
  return <SettingsApp page={page} onPage={setPage} />;
}

async function mount() {
  vi.mocked(api.getSettings).mockResolvedValue(settings);
  vi.mocked(api.listFeatures).mockResolvedValue({
    features: [{ id: "a" }, { id: "b" }, { id: "c" }] as never,
    onCount: 2,
  });
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Harness />));
  await act(async () => {});
}

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
  vi.clearAllMocks();
});

describe("SettingsApp", () => {
  it("shows the Features count", async () => {
    await mount();
    expect(screen.getByText("2/3 on")).toBeTruthy();
  });

  it("arrow keys move through the nav with roving tabindex", async () => {
    await mount();
    const general = screen.getByRole("button", { name: "General" });
    expect(general.getAttribute("aria-current")).toBe("page");
    expect(general.tabIndex).toBe(0);
    act(() => {
      fireEvent.keyDown(general, { key: "ArrowDown" });
    });
    const features = screen.getByRole("button", { name: "Appearance" });
    expect(document.activeElement).toBe(features);
    expect(features.getAttribute("aria-current")).toBe("page");
    expect(general.tabIndex).toBe(-1);
  });

  it("Ctrl+Tab and Ctrl+Shift+Tab step through the pages", async () => {
    await mount();
    act(() => {
      fireEvent.keyDown(window, { key: "Tab", ctrlKey: true });
    });
    expect(screen.getByRole("button", { name: "Appearance" }).getAttribute("aria-current")).toBe(
      "page",
    );
    act(() => {
      fireEvent.keyDown(window, { key: "Tab", ctrlKey: true, shiftKey: true });
    });
    expect(screen.getByRole("button", { name: "General" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("Esc and Ctrl+W close the window, even from a text field", async () => {
    await mount();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(ipc.closeWindow).toHaveBeenCalledTimes(1);
    const input = document.body.appendChild(document.createElement("input"));
    act(() => {
      fireEvent.keyDown(input, { key: "w", ctrlKey: true });
    });
    expect(ipc.closeWindow).toHaveBeenCalledTimes(2);
  });

  it("keys pressed while capturing a hotkey do not close Settings or change page", async () => {
    await mount();
    for (const o of [
      { key: "Escape" },
      { key: "w", ctrlKey: true },
      { key: "Tab", ctrlKey: true },
    ]) {
      act(() => {
        fireEvent.click(screen.getByText("Change"));
      });
      act(() => {
        fireEvent.keyDown(screen.getByText(/Press keys/), o);
      });
    }
    expect(ipc.closeWindow).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "General" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("the window handler ignores a key already handled", async () => {
    await mount();
    const ev = new KeyboardEvent("keydown", { key: "Escape", cancelable: true, bubbles: true });
    ev.preventDefault();
    act(() => {
      window.dispatchEvent(ev);
    });
    expect(ipc.closeWindow).not.toHaveBeenCalled();
  });

  it("page keys do not steal Ctrl+Tab while typing in a text field", async () => {
    await mount();
    const input = document.body.appendChild(document.createElement("input"));
    act(() => {
      fireEvent.keyDown(input, { key: "Tab", ctrlKey: true });
    });
    expect(screen.getByRole("button", { name: "General" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("clear history needs a second confirming click", async () => {
    await mount();
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Clear history" }));
    });
    expect(api.clearHistory).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Confirm clear" }));
    });
    expect(api.clearHistory).toHaveBeenCalledWith("all");
    expect(screen.queryByRole("button", { name: "Confirm clear" })).toBeNull();
  });
});

describe("resolveView", () => {
  it("maps sections, aliases and unknowns", () => {
    expect(resolveView("#first-run", null)).toBe("first-run");
    expect(resolveView("providers", "general")).toBe("providers");
    expect(resolveView("ai-providers", "general")).toBe("providers");
    expect(resolveView("nope", "about")).toBe("about");
    expect(resolveView(null, null)).toBeNull();
  });
});
