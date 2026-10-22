import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../ipc";
import type { ExplainCaptured, Item } from "../types";
import { Launcher } from "./Launcher";

vi.mock("../ipc", () => ({
  search: vi.fn(),
  launch: vi.fn(() => Promise.resolve({ type: "done" })),
  reveal: vi.fn(() => Promise.resolve()),
  copyPath: vi.fn(() => Promise.resolve()),
  runSystemCommand: vi.fn(),
  askStream: vi.fn(),
  cancelAsk: vi.fn(),
  listThinkingModes: vi.fn(() => Promise.resolve({ modes: [], selected: null })),
  setThinkingMode: vi.fn(() => Promise.resolve()),
  hideLauncher: vi.fn(() => Promise.resolve()),
  reportShown: vi.fn(() => Promise.resolve()),
  openSettings: vi.fn(),
  openLink: vi.fn(() => Promise.resolve()),
  copyText: vi.fn(() => Promise.resolve()),
  onShown: vi.fn(() => Promise.resolve(() => {})),
  onHidden: vi.fn(() => Promise.resolve(() => {})),
  onExplainCaptured: vi.fn(() => Promise.resolve(() => {})),
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
  localStorage.clear();
  act(() => root?.unmount());
  document.body.replaceChildren();
  vi.clearAllMocks();
});

/** Makes the next searches return one file row (id `file:a`). */
const mockFileResults = () =>
  vi.mocked(ipc.search).mockImplementation((req) =>
    Promise.resolve({
      seq: req.seq,
      items: [{ ...item("file:a", "Alpha"), kind: "file" }],
      group: null,
    }),
  );

const type = (text: string) =>
  act(async () => {
    fireEvent.change(screen.getByRole("combobox"), { target: { value: text } });
  });

describe("Launcher", () => {
  it("starts in none mode with four prefix hint rows and no search", async () => {
    await mount();
    expect(screen.getByPlaceholderText("Type / > = ? @ to start")).toBeTruthy();
    expect(screen.getAllByRole("option", { name: /^(\/|>|=|\?|@)/ })).toHaveLength(5);
    expect(screen.getByRole("listbox", { name: "Modes" })).toBeTruthy();
    expect(screen.getByText("> commands")).toBeTruthy();
    expect(ipc.search).not.toHaveBeenCalled();
    expect(screen.queryByTestId("mode-chip")).toBeNull();
  });

  it("shown with sources [apps] hides the other four prefix hint rows", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({ shownId: 1, reason: "hotkey", sources: ["apps"] });
      return Promise.resolve(() => {});
    });
    await mount();
    expect(screen.getAllByRole("option", { name: /^(\/|>|=|\?|@)/ })).toHaveLength(1);
    expect(screen.queryByText("> commands")).toBeNull();
  });

  it("plain text keeps the hints and shows a muted line", async () => {
    await mount();
    await type("abc");
    expect(screen.getByText("Start with / > = ? @ to search")).toBeTruthy();
    expect(ipc.search).not.toHaveBeenCalled();
  });

  it("typing a prefix shows a chip, the placeholder and searches with sources", async () => {
    await mount();
    await type("/");
    expect(screen.getByTestId("mode-chip").textContent).toBe("Open");
    expect(screen.getByRole("combobox").getAttribute("aria-label")).toBe("Open search");
    expect(screen.getByPlaceholderText("Open an app, file or folder")).toBeTruthy();
    expect(ipc.search).toHaveBeenCalledWith({
      seq: 1,
      mode: "find",
      text: "",
      sources: ["apps", "files", "nlfind"],
    });
    expect(screen.getAllByRole("option")).toHaveLength(2);
  });

  it("clicking a hint row enters its mode", async () => {
    await mount();
    act(() => {
      fireEvent.click(screen.getByRole("option", { name: /Open/ }));
    });
    expect(screen.getByTestId("mode-chip").textContent).toBe("Open");
  });

  it("shows group label, subtitle and mode footer", async () => {
    await mount();
    await type(">");
    expect(screen.getByText("Apps")).toBeTruthy();
    expect(screen.getAllByText("App")).toHaveLength(2);
    expect(screen.queryByText("Ctrl Shift ↵ admin")).toBeNull();
    expect(screen.queryByText("Tab mode")).toBeNull();
  });

  it("Open footer shows the admin hint when the selected item has an admin action", async () => {
    await mount();
    vi.mocked(ipc.search).mockImplementation((req) =>
      Promise.resolve({
        seq: req.seq,
        items: [{ ...item("a", "Alpha"), actions: [{ type: "admin" }] }],
        group: null,
      }),
    );
    await type("/");
    expect(screen.getByText("Ctrl Shift ↵ admin")).toBeTruthy();
  });

  it("labels drive-scan programs", async () => {
    await mount();
    vi.mocked(ipc.search).mockImplementation((req) =>
      Promise.resolve({
        seq: req.seq,
        items: [{ ...item("p", "Prog"), payload: { program: true }, subtitle: "D:/tools" }],
        group: null,
      }),
    );
    await type("/");
    expect(screen.getByText("Programs")).toBeTruthy();
    expect(screen.getByText("D:/tools")).toBeTruthy();
  });

  it("ArrowDown moves aria-activedescendant", async () => {
    await mount();
    await type(">");
    const input = screen.getByRole("combobox");
    expect(input.getAttribute("aria-activedescendant")).toBe("launcher-opt-0");
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    expect(input.getAttribute("aria-activedescendant")).toBe("launcher-opt-1");
  });

  it("Tab does not change the mode", async () => {
    await mount();
    await type(">");
    act(() => {
      fireEvent.keyDown(window, { key: "Tab" });
    });
    expect(screen.getByTestId("mode-chip").textContent).toBe("Commands");
  });

  it("Backspace on an empty input leaves the mode", async () => {
    await mount();
    await type(">");
    act(() => {
      fireEvent.keyDown(window, { key: "Backspace" });
    });
    expect(screen.queryByTestId("mode-chip")).toBeNull();
  });

  it("F1 opens the shortcut sheet and Esc closes it before anything else", async () => {
    await mount();
    await type(">");
    act(() => {
      fireEvent.keyDown(window, { key: "F1" });
    });
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeTruthy();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByTestId("mode-chip").textContent).toBe("Commands");
    expect(ipc.hideLauncher).not.toHaveBeenCalled();
  });

  it("Ctrl+Shift+C in Find copies the path, shows Copied, then clears it", async () => {
    vi.useFakeTimers();
    try {
      await mount();
      mockFileResults();
      await type("/");
      act(() => {
        fireEvent.keyDown(window, { key: "C", ctrlKey: true, shiftKey: true });
      });
      expect(ipc.copyPath).toHaveBeenCalledWith("file:a");
      expect(screen.queryByRole("status")).toBeNull();
      await act(async () => {});
      expect(screen.getByRole("status").textContent).toBe("Copied");
      act(() => {
        vi.advanceTimersByTime(1300);
      });
      expect(screen.queryByRole("status")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("a failed copy shows Copy failed, never Copied", async () => {
    vi.mocked(ipc.copyPath).mockRejectedValueOnce(new Error("clipboard busy"));
    const err = vi.spyOn(console, "error").mockImplementation(() => {});
    await mount();
    mockFileResults();
    await type("/");
    await act(async () => {
      fireEvent.keyDown(window, { key: "C", ctrlKey: true, shiftKey: true });
    });
    expect(screen.getByRole("status").textContent).toBe("Copy failed");
    err.mockRestore();
  });

  it("find footer advertises Ctrl Shift C on a file row only, not Ctrl C", async () => {
    await mount();
    await type("/");
    expect(screen.queryByText("Ctrl Shift C copy path")).toBeNull();
    mockFileResults();
    await type("/x");
    expect(screen.getAllByText("Ctrl Shift C copy path").length).toBeGreaterThan(0);
    expect(screen.queryByText("Ctrl C copy path")).toBeNull();
  });

  it("Escape on empty input hides the launcher", async () => {
    await mount();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(ipc.hideLauncher).toHaveBeenCalled();
  });
});

describe("Launcher explain", () => {
  async function capture(payload: ExplainCaptured) {
    await mount();
    const cb = vi.mocked(ipc.onExplainCaptured).mock.calls[0]?.[0];
    act(() => cb?.(payload));
  }

  it("shows the captured text before anything is sent, and Remove drops it", async () => {
    await capture({
      preview: "hello world",
      length: 11,
      source_app: "notepad.exe",
      blocked: false,
    });
    expect(screen.getByText("hello world")).toBeTruthy();
    expect(screen.getByText(/notepad\.exe/)).toBeTruthy();
    expect(ipc.askStream).not.toHaveBeenCalled();
    act(() => {
      fireEvent.click(screen.getByText("Remove"));
    });
    expect(screen.queryByText("hello world")).toBeNull();
  });

  it("blocked text shows why and Enter does not ask", async () => {
    await capture({
      preview: "",
      length: 40,
      source_app: "x.exe",
      blocked: true,
      reason: "looks like a secret",
    });
    expect(screen.getByRole("alert").textContent).toContain("looks like a secret");
    act(() => {
      fireEvent.keyDown(window, { key: "Enter" });
    });
    expect(ipc.askStream).not.toHaveBeenCalled();
  });

  it("nothing selected shows a hint", async () => {
    await capture({ preview: "", length: 0, source_app: "x.exe", blocked: false });
    expect(screen.getByText("No text selected")).toBeTruthy();
  });

  it("shows a failed launch as an alert and stays open", async () => {
    await mount();
    await type(">al");
    vi.mocked(ipc.launch).mockRejectedValueOnce({
      code: "failed",
      message: "Sleep is not supported",
    });
    await act(async () => {
      fireEvent.keyDown(window, { key: "Enter" });
    });
    expect(screen.getByRole("alert").textContent).toBe("Sleep is not supported");
    expect(ipc.hideLauncher).not.toHaveBeenCalled();
    await type(">alp");
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
