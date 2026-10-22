import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../ipc";
import type { ExplainCaptured, Item, ItemKind } from "../types";
import { footHints } from "./Footer";
import { KindIcon } from "./KindIcon";
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
  webSearch: vi.fn(() => Promise.resolve({ type: "done" })),
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
    expect(screen.getByPlaceholderText("Search apps and commands, or type / > = ? @")).toBeTruthy();
    expect(screen.getByText("Type without a prefix to search apps and commands")).toBeTruthy();
    expect(screen.getAllByRole("option", { name: /^(\/|>|=|\?|@)/ })).toHaveLength(5);
    expect(screen.getByRole("listbox", { name: "Modes" })).toBeTruthy();
    expect(screen.getByText("commands")).toBeTruthy();
    expect(ipc.search).not.toHaveBeenCalled();
    expect(screen.queryByTestId("mode-chip")).toBeNull();
  });

  it("hint rows carry Ctrl+N caps and the list has one aria-hidden lens", async () => {
    await mount();
    const rows = screen.getAllByRole("option", { name: /^(\/|>|=|\?|@)/ });
    const caps = rows.map((r) =>
      [...r.querySelectorAll("kbd")].map((k) => k.textContent).join(" "),
    );
    expect(caps).toEqual(["Ctrl 1", "Ctrl 2", "Ctrl 3", "Ctrl 4", "Ctrl 5"]);
    expect(screen.getByText("Narrow with a prefix")).toBeTruthy();
    const lenses = document.querySelectorAll(".lens");
    expect(lenses).toHaveLength(1);
    expect(lenses[0]?.getAttribute("aria-hidden")).toBe("true");
  });

  it("ArrowDown moves aria-selected and aria-activedescendant, with one lens", async () => {
    await mount();
    await type(">");
    const input = screen.getByRole("combobox");
    const before = input.getAttribute("aria-activedescendant");
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    expect(input.getAttribute("aria-activedescendant")).not.toBe(before);
    const selected = screen
      .getAllByRole("option")
      .filter((o) => o.getAttribute("aria-selected") === "true");
    expect(selected).toHaveLength(1);
    expect(selected[0]?.id).toBe(input.getAttribute("aria-activedescendant"));
    expect(document.querySelectorAll(".lens")).toHaveLength(1);
  });

  it("file rows render a kind icon tile and app rows without an icon a letter tile", async () => {
    await mount();
    await type(">");
    expect(document.querySelectorAll(".tile-letter").length).toBeGreaterThan(0);
    mockFileResults();
    await type("/x");
    expect(document.querySelector(".row .tile-ico svg")).not.toBeNull();
  });

  it("shown with sources [apps] hides the other four prefix hint rows", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({
        shownId: 1,
        reason: "hotkey",
        sources: ["apps"],
        appearance: { theme: "glass", glass: 90, scheme: "dark" },
      });
      return Promise.resolve(() => {});
    });
    await mount();
    expect(screen.getAllByRole("option", { name: /^(\/|>|=|\?|@)/ })).toHaveLength(1);
    expect(screen.queryByText("commands")).toBeNull();
  });

  it("plain text searches apps and features and lists the Search instead rows", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({
        shownId: 1,
        reason: "hotkey",
        sources: ["apps", "files", "ask"],
        webTitle: "Google",
        appearance: { theme: "glass", glass: 90, scheme: "dark" },
      });
      return Promise.resolve(() => {});
    });
    await mount();
    await type("abc");
    expect(ipc.search).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "command", sources: ["apps", "features"], text: "abc" }),
    );
    expect(screen.queryByRole("listbox", { name: "Modes" })).toBeNull();
    expect(screen.getByText("Search instead")).toBeTruthy();
    const titles = [...document.querySelectorAll(".row-title")].map((t) => t.textContent);
    expect(titles.slice(-3)).toEqual([
      "Find files named “abc”",
      "Search Google for “abc”",
      "Ask your AI: “abc”",
    ]);
    expect(document.querySelectorAll(".row .tile-glyph")).toHaveLength(3);
    const rows = [...document.querySelectorAll(".row")].slice(-3);
    expect(rows.every((r) => r.querySelector(".row-sub") === null)).toBe(true);
    expect(rows.every((r) => r.querySelector(".row-meta") === null)).toBe(true);
    expect(screen.queryByText("No matches")).toBeNull();
  });

  it("Enter on the Find row enters Open carrying the text", async () => {
    await mount();
    await type("abc");
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    act(() => {
      fireEvent.keyDown(window, { key: "Enter" });
    });
    expect(screen.getByTestId("mode-chip").textContent).toContain("Open");
    expect(screen.getByRole<HTMLInputElement>("combobox").value).toBe("abc");
  });

  it("typing a prefix shows a chip, the placeholder and searches with sources", async () => {
    await mount();
    await type("/");
    expect(screen.getByTestId("mode-chip").textContent).toBe("/Open");
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
    expect(screen.getByTestId("mode-chip").textContent).toBe("/Open");
  });

  it("shows group label, subtitle and mode footer", async () => {
    await mount();
    await type(">");
    expect(screen.getByText("Apps")).toBeTruthy();
    expect(screen.getAllByText("App")).toHaveLength(2);
    expect(screen.queryByText("admin")).toBeNull();
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
    const act = screen.getByText("admin");
    expect(act.className).toBe("act");
    expect([...act.querySelectorAll("kbd")].map((k) => k.textContent)).toEqual([
      "Ctrl",
      "Shift",
      "↵",
    ]);
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
    expect(screen.getByTestId("mode-chip").textContent).toBe(">Commands");
  });

  it("Backspace on an empty input leaves the mode", async () => {
    await mount();
    await type(">");
    act(() => {
      fireEvent.keyDown(window, { key: "Backspace" });
    });
    expect(screen.queryByTestId("mode-chip")).toBeNull();
  });

  it("the chip holds the prefix glyph, and the Settings hint shows in none mode only", async () => {
    await mount();
    expect(screen.getByText("Settings")).toBeTruthy();
    await type("/");
    expect(screen.getByTestId("mode-chip").querySelector(".glyph")?.textContent).toBe("/");
    expect(screen.queryByText("Settings")).toBeNull();
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
    expect(screen.getByTestId("mode-chip").textContent).toBe(">Commands");
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
    expect(screen.queryByText("copy path")).toBeNull();
    mockFileResults();
    await type("/x");
    const acts = screen.getAllByText("copy path");
    expect([...acts[0].querySelectorAll("kbd")].map((k) => k.textContent)).toEqual([
      "Ctrl",
      "Shift",
      "C",
    ]);
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

describe("footHints", () => {
  it("find lists open and location, and admin only when the selection has it", () => {
    expect(footHints("find").map((h) => h.label)).toEqual(["open"]);
    expect(footHints("find", { revealable: true }).map((h) => h.label)).toEqual([
      "open",
      "location",
    ]);
    const admin = footHints("find", { adminAction: true }).find((h) => h.id === "admin");
    expect(admin?.caps).toEqual(["Ctrl", "Shift", "↵"]);
  });

  it("commands lists a single Enter hint without copy until a value exists", () => {
    expect(footHints("commands").map((h) => h.caps)).toEqual([["↵"]]);
  });

  it("none lists the prefixes as one cap each, then the shortcut sheet", () => {
    const hints = footHints("none", {}, ["find", "calc"]);
    expect(hints.map((h) => h.id)).toEqual(["go:find", "go:calc", "sheet"]);
    expect(hints[0]).toEqual({ id: "go:find", caps: ["/"], label: "open" });
    expect(hints[2]?.label).toBe("shortcuts");
  });
});

describe("KindIcon", () => {
  it("draws an svg for every kind", () => {
    const kinds: ItemKind[] = [
      "app",
      "command",
      "file",
      "folder",
      "project",
      "result",
      "process",
      "media",
      "capture",
      "choice",
    ];
    for (const kind of kinds) {
      expect(renderToStaticMarkup(<KindIcon kind={kind} />)).toContain("<svg");
    }
  });
});
