import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "./ipc";
import { Root } from "./Root";

vi.mock("./ipc", () => ({
  search: vi.fn(() => Promise.resolve({ seq: 0, items: [], group: null })),
  launch: vi.fn(),
  askStream: vi.fn(),
  cancelAsk: vi.fn(),
  listThinkingModes: vi.fn(() => Promise.resolve({ modes: [], selected: null })),
  setThinkingMode: vi.fn(() => Promise.resolve()),
  homeItems: vi.fn(() => Promise.resolve({ items: [], pins: [] })),
  hideLauncher: vi.fn(() => Promise.resolve()),
  resizeLauncher: vi.fn(() => Promise.resolve()),
  reportShown: vi.fn(() => Promise.resolve()),
  openSettings: vi.fn(() => Promise.resolve()),
  trayAction: vi.fn(() => Promise.resolve()),
  onShown: vi.fn(() => Promise.resolve(() => {})),
  onHidden: vi.fn(() => Promise.resolve(() => {})),
  onTrayMenu: vi.fn(() => Promise.resolve(() => {})),
  onExplainCaptured: vi.fn(() => Promise.resolve(() => {})),
}));

const event: ipc.TrayMenuEvent = { appVersion: "0.0.1", hotkey: "Alt+Space", update: null };

async function mount() {
  let showMenu: (e: ipc.TrayMenuEvent) => void = () => {};
  let hidden: () => void = () => {};
  vi.mocked(ipc.onTrayMenu).mockImplementation((cb) => {
    showMenu = cb;
    return Promise.resolve(() => {});
  });
  vi.mocked(ipc.onHidden).mockImplementation((cb) => {
    hidden = cb;
    return Promise.resolve(() => {});
  });
  render(<Root />);
  await act(async () => {});
  return {
    showMenu: () => act(() => showMenu(event)),
    hidden: () => act(() => hidden()),
  };
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
  localStorage.clear();
  vi.clearAllMocks();
});

describe("Root", () => {
  it("shows the launcher first", async () => {
    await mount();
    expect(screen.getByRole("combobox")).toBeTruthy();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("swaps to the menu on tray://menu and back on launcher://hidden", async () => {
    const m = await mount();
    m.showMenu();
    expect(screen.getByRole("menu")).toBeTruthy();
    expect(screen.queryByRole("combobox")).toBeNull();
    m.hidden();
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("combobox")).toBeTruthy();
  });

  it("replays a show that arrives during the menu into the launcher", async () => {
    let shown: (p: ipc.ShownEvent) => void = () => {};
    vi.mocked(ipc.onShown).mockImplementation((cb) => {
      shown = cb;
      return Promise.resolve(() => {});
    });
    const m = await mount();
    m.showMenu();
    act(() => shown({ shownId: 7, reason: "hotkey", sources: ["apps"] }));
    await act(async () => {
      await new Promise((r) => requestAnimationFrame(r));
    });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("tab", { name: "all" }).getAttribute("aria-selected")).toBe("true");
    expect(ipc.reportShown).toHaveBeenCalledTimes(1);
    expect(ipc.reportShown).toHaveBeenCalledWith(7);
  });

  it("runs the chosen action through the tray_action command", async () => {
    const m = await mount();
    m.showMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/ }));
    expect(ipc.trayAction).toHaveBeenCalledWith("settings");
  });

  it("Escape hides the launcher window", async () => {
    const m = await mount();
    m.showMenu();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(ipc.hideLauncher).toHaveBeenCalled();
  });
});
