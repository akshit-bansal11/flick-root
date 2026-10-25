import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../ipc";
import type { Appearance, ThemeRoles, ThemeView } from "../theme";
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
  homeItems: vi.fn(() => Promise.resolve({ items: [], pins: [] })),
  togglePin: vi.fn(() => Promise.resolve({ items: [], pins: [] })),
  setThinkingMode: vi.fn(() => Promise.resolve()),
  hideLauncher: vi.fn(() => Promise.resolve()),
  reportShown: vi.fn(() => Promise.resolve()),
  openSettings: vi.fn(),
  openLink: vi.fn(() => Promise.resolve()),
  webSearch: vi.fn(() => Promise.resolve({ type: "done" })),
  copyText: vi.fn(() => Promise.resolve()),
  listThemes: vi.fn(() => Promise.resolve([])),
  setAppearance: vi.fn(),
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
const C = "#123456";
const ROLE_FIXTURE: ThemeRoles = {
  bg: C,
  panel: C,
  surface: C,
  line: C,
  text: C,
  muted: C,
  accent: C,
  accentText: C,
  onAccent: C,
  sel: C,
  ok: C,
  warn: C,
  err: C,
};
const APPEARANCE: Appearance = {
  scheme: "dark",
  darkTheme: "t",
  lightTheme: "t",
  active: { id: "t", dark: true, roles: ROLE_FIXTURE },
};

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
  vi.mocked(ipc.homeItems).mockResolvedValue({ items: [], pins: [] });
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
  it("starts on the all tab with the mode tab strip and no search", async () => {
    await mount();
    expect(screen.getByPlaceholderText("Search apps and commands, or type / > = ? @")).toBeTruthy();
    expect(screen.getByRole("tablist", { name: "Modes" })).toBeTruthy();
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual([
      "all",
      "/open",
      ">commands",
      "=calc",
      "?ask",
      "@projects",
    ]);
    expect(screen.getByRole("tab", { name: "all" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByRole("listbox", { name: "Modes" })).toBeNull();
    expect(ipc.search).not.toHaveBeenCalled();
    expect(screen.getByTestId("prompt").textContent).toBe("›");
  });

  it("shown loads the recent picks and lists them under # recent", async () => {
    vi.mocked(ipc.homeItems).mockResolvedValue({
      items: [{ ...item("r1", "Recent One"), payload: { home: "recent" } }],
      pins: [],
    });
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({ shownId: 1, reason: "hotkey", sources: ["apps"], appearance: APPEARANCE });
      return Promise.resolve(() => {});
    });
    await mount();
    expect(ipc.homeItems).toHaveBeenCalled();
    expect(screen.getByText("# recent")).toBeTruthy();
    expect(screen.getByRole("option", { name: /Recent One/ })).toBeTruthy();
    expect(ipc.search).not.toHaveBeenCalled();
  });

  describe("pins", () => {
    const rowsOf = (cls: string) => [...document.querySelectorAll(cls)].map((e) => e.textContent);
    const showHome = (items: Item[], pins: string[]) => {
      vi.mocked(ipc.homeItems).mockResolvedValue({ items, pins });
      vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
        cb({ shownId: 1, reason: "hotkey", sources: ["apps"], appearance: APPEARANCE });
        return Promise.resolve(() => {});
      });
    };
    const recent = (id: string, title: string): Item => ({
      ...item(id, title),
      payload: { home: "recent" },
    });

    it("Ctrl+P on an app row toggles its pin and relists # pinned before # recent", async () => {
      showHome([recent("app:r1", "Recent One"), recent("app:r2", "Recent Two")], []);
      await mount();
      expect(screen.queryByText("# pinned")).toBeNull();
      vi.mocked(ipc.togglePin).mockResolvedValueOnce({
        items: [
          { ...item("app:r2", "Recent Two"), payload: { home: "pinned" } },
          recent("app:r1", "Recent One"),
        ],
        pins: ["app:r2"],
      });
      await act(async () => {
        fireEvent.keyDown(window, { key: "p", ctrlKey: true });
      });
      expect(ipc.togglePin).toHaveBeenCalledWith("app:r1");
      const headers = rowsOf(".group");
      expect(headers).toEqual(["# pinned", "# recent"]);
      expect(rowsOf("[role=option]")[0]).toContain("Recent Two");
    });

    it("Ctrl+P on a row that cannot be pinned does not call togglePin", async () => {
      showHome([{ ...recent("cmd:x", "Lock"), kind: "command" }], []);
      await mount();
      await act(async () => {
        fireEvent.keyDown(window, { key: "p", ctrlKey: true });
      });
      expect(ipc.togglePin).not.toHaveBeenCalled();
    });

    it("a rejected togglePin shows its message as the notice", async () => {
      showHome([recent("app:r1", "Recent One")], []);
      await mount();
      const err = vi.spyOn(console, "error").mockImplementation(() => {});
      vi.mocked(ipc.togglePin).mockRejectedValueOnce({ code: "pin_failed", message: "Pin limit" });
      await act(async () => {
        fireEvent.keyDown(window, { key: "p", ctrlKey: true });
      });
      expect(screen.getByRole("alert").textContent).toBe("Pin limit");
      err.mockRestore();
    });
  });

  it("the selected tab follows the mode and a tab click switches it", async () => {
    await mount();
    const tab = (name: string) => screen.getByRole("tab", { name });
    act(() => {
      fireEvent.click(tab("/open"));
    });
    expect(tab("/open").getAttribute("aria-selected")).toBe("true");
    expect(tab("all").getAttribute("aria-selected")).toBe("false");
    await type(">x");
    expect(tab(">commands").getAttribute("aria-selected")).toBe("true");
    expect(tab("/open").getAttribute("aria-selected")).toBe("false");
    act(() => {
      fireEvent.click(tab("all"));
    });
    expect(tab("all").getAttribute("aria-selected")).toBe("true");
    expect(screen.getByTestId("prompt").textContent).toBe("›");
  });

  it("file rows carry their kind tag", async () => {
    await mount();
    mockFileResults();
    await type("/x");
    expect(document.querySelector(".row .row-meta")?.textContent).toBe("file");
  });

  it("shown with sources [apps] leaves only the all and open tabs", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({
        shownId: 1,
        reason: "hotkey",
        sources: ["apps"],
        appearance: APPEARANCE,
      });
      return Promise.resolve(() => {});
    });
    await mount();
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["all", "/open"]);
    expect(screen.queryByRole("tab", { name: ">commands" })).toBeNull();
  });

  it("shown payload with active colours reaches documentElement.style", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({ shownId: 1, reason: "hotkey", sources: ["apps"], appearance: APPEARANCE });
      return Promise.resolve(() => {});
    });
    await mount();
    expect(document.documentElement.style.getPropertyValue("--l-accent")).toBe("#123456");
    expect(document.documentElement.dataset.scheme).toBe("dark");
  });

  it("plain text searches apps and features and lists the Search instead rows", async () => {
    vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
      cb({
        shownId: 1,
        reason: "hotkey",
        sources: ["apps", "files", "ask"],
        webTitle: "Google",
        appearance: APPEARANCE,
      });
      return Promise.resolve(() => {});
    });
    await mount();
    await type("abc");
    expect(ipc.search).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "command", sources: ["apps", "features"], text: "abc" }),
    );
    expect(screen.queryByRole("listbox", { name: "Modes" })).toBeNull();
    expect(screen.getByText("# search instead")).toBeTruthy();
    const titles = [...document.querySelectorAll(".row-title")].map((t) => t.textContent);
    expect(titles.slice(-3)).toEqual([
      "Find files named “abc”",
      "Search Google for “abc”",
      "Ask your AI: “abc”",
    ]);
    const rows = [...document.querySelectorAll(".row")].slice(-3);
    expect(rows.every((r) => r.querySelector(".row-sub") === null)).toBe(true);
    expect(rows.every((r) => r.querySelector(".row-meta") === null)).toBe(true);
    expect(screen.queryByText("No matches")).toBeNull();
  });

  it("the › marker is only on the selected row and moves with ArrowDown", async () => {
    await mount();
    await type("abc");
    const marks = () =>
      [...document.querySelectorAll(".row")].map((r) => r.querySelector(".row-mark")?.textContent);
    expect(marks()[0]).toBe("›");
    expect(marks().filter((m) => m === "›")).toHaveLength(1);
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    expect(marks()[0]).toBe("");
    expect(marks()[1]).toBe("›");
    expect(marks().filter((m) => m === "›")).toHaveLength(1);
  });

  it("the group header reads # apps", async () => {
    await mount();
    await type("abc");
    expect(screen.getByText("# apps")).toBeTruthy();
  });

  it("aria-activedescendant follows the selection", async () => {
    await mount();
    await type("abc");
    const active = () => screen.getByRole("combobox").getAttribute("aria-activedescendant");
    const options = screen.getAllByRole("option");
    expect(active()).toBe(options[0]?.id);
    act(() => {
      fireEvent.keyDown(window, { key: "ArrowDown" });
    });
    expect(active()).toBe(options[1]?.id);
  });

  it("matched text is wrapped in mark", async () => {
    await mount();
    await type("alp");
    const row = screen.getByRole("option", { name: /Alpha/ });
    const marked = [...row.querySelectorAll(".row-title mark")].map((m) => m.textContent);
    expect(marked).toEqual(["Alp"]);
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
    expect(screen.getByTestId("prompt").textContent).toBe("/");
    expect(screen.getByRole<HTMLInputElement>("combobox").value).toBe("abc");
  });

  it("typing a prefix shows its glyph, the placeholder and searches with sources", async () => {
    await mount();
    await type("/");
    expect(screen.getByTestId("prompt").textContent).toBe("/");
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

  it("clicking a tab enters its mode", async () => {
    await mount();
    act(() => {
      fireEvent.click(screen.getByRole("tab", { name: "/open" }));
    });
    expect(screen.getByTestId("prompt").textContent).toBe("/");
  });

  it("shows group label, subtitle and mode footer", async () => {
    await mount();
    await type(">");
    expect(screen.getByText("# apps")).toBeTruthy();
    const tags = [...document.querySelectorAll(".row-meta")].map((t) => t.textContent);
    expect(tags).toEqual(["app", "app"]);
    expect(screen.queryByText("admin")).toBeNull();
    expect(screen.queryByText("Tab mode")).toBeNull();
  });

  it("detail pane shows the admin action with its chord", async () => {
    await mount();
    vi.mocked(ipc.search).mockImplementation((req) =>
      Promise.resolve({
        seq: req.seq,
        items: [{ ...item("a", "Alpha"), actions: [{ type: "admin" }] }],
        group: null,
      }),
    );
    await type("/");
    const btn = screen.getByRole("button", { name: /run as administrator/i });
    expect(btn.querySelector(".detail-keys")?.textContent).toBe("ctrl+shift+enter");
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
    expect(screen.getByText("# programs")).toBeTruthy();
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
    expect(screen.getByTestId("prompt").textContent).toBe(">");
  });

  it("Backspace on an empty input leaves the mode", async () => {
    await mount();
    await type(">");
    act(() => {
      fireEvent.keyDown(window, { key: "Backspace" });
    });
    expect(screen.getByTestId("prompt").textContent).toBe("›");
    expect(screen.getByRole("tab", { name: "all" }).getAttribute("aria-selected")).toBe("true");
  });

  it.each(["/", ">", "=", "?", "@"])("the prompt shows %s in its mode", async (prefix) => {
    await mount();
    await type(prefix);
    expect(screen.getByTestId("prompt").textContent).toBe(prefix);
  });

  it("the bar note says settings on all", async () => {
    await mount();
    expect(document.querySelector(".inputbar .bar-note")?.textContent).toBe("ctrl+, settings");
  });

  it("the bar note is absent in a mode with no text", async () => {
    await mount();
    await type("/");
    expect(document.querySelector(".inputbar .bar-note")).toBeNull();
  });

  it("the bar note says clear once text is typed in a mode", async () => {
    await mount();
    await type("/abc");
    expect(document.querySelector(".inputbar .bar-note")?.textContent).toBe("esc clear");
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
    expect(screen.getByTestId("prompt").textContent).toBe(">");
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

  it("find footer advertises ctrl+shift+c on a file row only, not Ctrl C", async () => {
    await mount();
    await type("/");
    const hints = () =>
      [...document.querySelectorAll(".foot .hint-keys")].map((k) => k.textContent);
    expect(hints()).not.toContain("ctrl+shift+c");
    mockFileResults();
    await type("/x");
    expect(hints()).toContain("ctrl+shift+c");
    expect(hints()).not.toContain("ctrl+c");
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

  describe("> theme quick pick", () => {
    const view = (id: string, name: string, accent: string): ThemeView => ({
      id,
      name,
      source: "builtin",
      dark: true,
      roles: { ...ROLE_FIXTURE, accent },
      ratios: {
        text: 7,
        muted: 4.5,
        accentText: 4.5,
        onAccent: 4.5,
        selText: 7,
        mutedBg: 4.5,
        mutedSel: 4.5,
      },
      adjusted: [],
      upstream: null,
    });
    const accent = () => document.documentElement.style.getPropertyValue("--l-accent");
    const arrow = (key: string) =>
      act(() => {
        fireEvent.keyDown(window, { key });
      });

    async function openPick() {
      vi.mocked(ipc.listThemes).mockResolvedValue([
        view("t", "Saved", C),
        view("u", "Other", "#00aa00"),
      ]);
      vi.mocked(ipc.onShown).mockImplementationOnce((cb) => {
        cb({ shownId: 1, reason: "hotkey", appearance: APPEARANCE });
        return Promise.resolve(() => {});
      });
      await mount();
      await type(">theme");
      await act(async () => {});
    }

    it("arrows repaint without another list_themes or any set_appearance", async () => {
      await openPick();
      expect(screen.getAllByRole("option")).toHaveLength(2);
      expect(accent()).toBe(C);
      arrow("ArrowDown");
      expect(accent()).toBe("#00aa00");
      arrow("ArrowDown");
      expect(accent()).toBe(C);
      expect(ipc.listThemes).toHaveBeenCalledTimes(1);
      expect(ipc.setAppearance).not.toHaveBeenCalled();
    });

    it("Escape puts the saved look back and keeps the pick open", async () => {
      await openPick();
      arrow("ArrowDown");
      expect(accent()).toBe("#00aa00");
      arrow("Escape");
      expect(accent()).toBe(C);
      expect(screen.getAllByRole("option")).toHaveLength(2);
      expect(ipc.hideLauncher).not.toHaveBeenCalled();
    });
  });
});
