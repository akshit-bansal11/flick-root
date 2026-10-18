import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../ipc";
import type { Item } from "../types";
import { Launcher } from "./Launcher";

vi.mock("../ipc", () => ({
  search: vi.fn(),
  launch: vi.fn(() => Promise.resolve({ type: "done" })),
  reveal: vi.fn(() => Promise.resolve()),
  copyPath: vi.fn(() => Promise.resolve()),
  runSystemCommand: vi.fn(),
  askStream: vi.fn(),
  cancelAsk: vi.fn(),
  hideLauncher: vi.fn(() => Promise.resolve()),
  resizeLauncher: vi.fn(() => Promise.resolve()),
  reportShown: vi.fn(() => Promise.resolve()),
  openSettings: vi.fn(),
  onShown: vi.fn(() => Promise.resolve(() => {})),
  onHidden: vi.fn(() => Promise.resolve(() => {})),
}));

const item = (id: string, title: string): Item => ({
  id,
  kind: "app",
  title,
  subtitle: "App",
  icon: null,
  score: 1,
  source: "apps",
  actions: [],
  payload: null,
});
const items = [item("a", "Alpha"), item("b", "Beta")];

let root: Root | undefined;

async function mount() {
  vi.mocked(ipc.search).mockImplementation((req) =>
    Promise.resolve({ seq: req.seq, items, group: null }),
  );
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Launcher />));
  await act(async () => {});
}

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
  vi.clearAllMocks();
});

describe("Launcher", () => {
  it("renders the command placeholder and results as options", async () => {
    await mount();
    expect(screen.getByPlaceholderText("Open an app or run a command")).toBeTruthy();
    expect(screen.getAllByRole("option")).toHaveLength(2);
  });

  it("ArrowDown moves aria-activedescendant", async () => {
    await mount();
    const input = screen.getByRole("combobox");
    expect(input.getAttribute("aria-activedescendant")).toBe("launcher-opt-0");
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    expect(input.getAttribute("aria-activedescendant")).toBe("launcher-opt-1");
  });

  it("Tab changes the mode hint and placeholder", async () => {
    await mount();
    expect(screen.getByTestId("mode-hint").textContent).toContain("Command");
    act(() => {
      fireEvent.keyDown(window, { key: "Tab" });
    });
    expect(screen.getByTestId("mode-hint").textContent).toContain("Find");
    expect(screen.getByPlaceholderText("Find a file or folder")).toBeTruthy();
  });

  it("Escape on empty input hides the launcher", async () => {
    await mount();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(ipc.hideLauncher).toHaveBeenCalled();
  });
});
