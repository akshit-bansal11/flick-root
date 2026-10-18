import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Launcher } from "./Launcher";

vi.mock("../ipc", () => ({
  search: vi.fn(() => Promise.resolve({ seq: 0, items: [], group: null })),
  hideLauncher: vi.fn(() => Promise.resolve()),
  resizeLauncher: vi.fn(() => Promise.resolve()),
  reportShown: vi.fn(() => Promise.resolve()),
  onShown: vi.fn(() => Promise.resolve(() => {})),
  onHidden: vi.fn(() => Promise.resolve(() => {})),
}));

const KEY = "prefixes-overlay-seen";
let root: Root | undefined;

async function mount() {
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Launcher />));
  await act(async () => {});
}
const input = () => screen.getByRole("combobox") as HTMLInputElement;

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  localStorage.clear();
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe("prefixes overlay", () => {
  it("shows when the flag is absent", async () => {
    await mount();
    expect(screen.getByRole("note", { name: "Mode prefixes" })).toBeTruthy();
  });

  it("any key dismisses, sets the flag and is not forwarded", async () => {
    await mount();
    fireEvent.keyDown(window, { key: ">", shiftKey: true });
    expect(screen.queryByRole("note")).toBeNull();
    expect(localStorage.getItem(KEY)).toBe("1");
    expect(input().value).toBe("");
    expect(input().placeholder).toBe("Open an app or run a command");
  });

  it("is not shown when the flag is set", async () => {
    localStorage.setItem(KEY, "1");
    await mount();
    expect(screen.queryByRole("note")).toBeNull();
  });

  it("still dismisses when storage throws", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    await mount();
    expect(screen.getByRole("note")).toBeTruthy();
    fireEvent.keyDown(window, { key: "a" });
    expect(screen.queryByRole("note")).toBeNull();
  });
});
