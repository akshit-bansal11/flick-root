import {
  cleanup,
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type ActiveTheme, applyTheme, ROLE_VARS, type ThemeRoles, type ThemeView } from "../theme";
import { Appearance } from "./Appearance";
import * as api from "./api";
import type { AppearanceView } from "./types";

vi.mock("./api", () => ({
  getAppearance: vi.fn(),
  setAppearance: vi.fn(),
  listThemes: vi.fn(),
}));

const BASE: ThemeRoles = {
  bg: "#000000",
  panel: "#111111",
  surface: "#222222",
  line: "#333333",
  text: "#ffffff",
  muted: "#999999",
  accent: "#3399ff",
  accentText: "#66bbff",
  onAccent: "#000000",
  sel: "#223344",
  ok: "#00cc66",
  warn: "#ffaa00",
  err: "#ff4444",
};

const theme = (
  id: string,
  name: string,
  source: ThemeView["source"],
  dark: boolean,
  bg: string,
  text: number,
  adjusted: string[] = [],
): ThemeView => ({
  id,
  name,
  source,
  dark,
  roles: { ...BASE, bg },
  ratios: {
    text,
    muted: text - 4,
    accentText: text - 5,
    onAccent: text - 3,
    selText: text,
    mutedBg: 4.5,
    mutedSel: 4.5,
  },
  adjusted,
  upstream: source === "port" ? "github.com/example/harbor" : null,
});

const D1 = theme("d1", "Night", "builtin", true, "#0a0b0c", 12.3);
const D2 = theme("d2", "Harbor", "port", true, "#010203", 15.8, ["muted"]);
const L1 = theme("l1", "Paper", "builtin", false, "#f0f1f2", 13.4);
const L2 = theme("l2", "Sage", "imported", false, "#e0e1e2", 10.2);
const THEMES = [D1, D2, L1, L2];

const activeOf = (t: ThemeView): ActiveTheme => ({ id: t.id, dark: t.dark, roles: t.roles });
const cur: AppearanceView = {
  scheme: "system",
  darkTheme: "d1",
  lightTheme: "l1",
  active: activeOf(D1),
};

const bg = () => document.documentElement.style.getPropertyValue(ROLE_VARS.bg);
const changeButtons = () => screen.getAllByRole("button", { name: "change" });
const openDark = async () => {
  fireEvent.click((await screen.findAllByRole("button", { name: "change" }))[0]);
  return screen.findByRole("combobox", { name: "Filter themes" });
};
const optionNames = () =>
  screen.getAllByRole("option").map((o) => o.querySelector("b")?.textContent);

beforeEach(() => {
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  vi.mocked(api.getAppearance).mockResolvedValue({ ...cur });
  vi.mocked(api.listThemes).mockResolvedValue(THEMES);
  vi.mocked(api.setAppearance).mockImplementation((p) => Promise.resolve({ ...cur, ...p }));
  applyTheme(document.documentElement, activeOf(D1));
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("Appearance page", () => {
  it("a scheme button saves { scheme }", async () => {
    render(<Appearance />);
    const group = await screen.findByRole("group", { name: "Colour scheme" });
    expect(within(group).getByRole("button", { name: "system" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    fireEvent.click(within(group).getByRole("button", { name: "dark" }));
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ scheme: "dark" }));
  });

  it("shows the current theme names and no picker until change is pressed", async () => {
    render(<Appearance />);
    await screen.findAllByRole("button", { name: "change" });
    expect(screen.getByText("Night")).toBeTruthy();
    expect(screen.getByText("Paper")).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Filter themes" })).toBeNull();
  });

  it("change opens the picker for the dark slot", async () => {
    render(<Appearance />);
    await openDark();
    expect(screen.getByRole("group", { name: "Choose dark theme" })).toBeTruthy();
    expect(optionNames()).toEqual(["Night", "Harbor"]);
  });

  it("change opens the picker for the light slot", async () => {
    render(<Appearance />);
    await screen.findAllByRole("button", { name: "change" });
    fireEvent.click(changeButtons()[1]);
    expect(await screen.findByRole("group", { name: "Choose light theme" })).toBeTruthy();
    expect(optionNames()).toEqual(["Paper", "Sage"]);
  });
});

describe("theme picker", () => {
  it("ArrowDown previews on the document root and never calls setAppearance", async () => {
    render(<Appearance />);
    const input = await openDark();
    expect(bg()).toBe("#0a0b0c");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(bg()).toBe("#010203");
    expect(api.setAppearance).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(bg()).toBe("#0a0b0c");
    expect(api.setAppearance).not.toHaveBeenCalled();
  });

  it("Enter saves the dark slot and applies the returned active theme", async () => {
    vi.mocked(api.setAppearance).mockResolvedValue({
      ...cur,
      darkTheme: "d2",
      active: activeOf(D2),
    });
    render(<Appearance />);
    const input = await openDark();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ darkTheme: "d2" }));
    expect(api.setAppearance).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(bg()).toBe("#010203"));
    await waitFor(() =>
      expect(screen.queryByRole("combobox", { name: "Filter themes" })).toBeNull(),
    );
  });

  it("Enter in the light picker saves the light slot", async () => {
    render(<Appearance />);
    await screen.findAllByRole("button", { name: "change" });
    fireEvent.click(changeButtons()[1]);
    const input = await screen.findByRole("combobox", { name: "Filter themes" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ lightTheme: "l2" }));
  });

  it("Escape restores the saved colours, closes, and saves nothing", async () => {
    render(<Appearance />);
    const input = await openDark();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(bg()).toBe("#010203");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(bg()).toBe("#0a0b0c");
    expect(screen.queryByRole("combobox", { name: "Filter themes" })).toBeNull();
    expect(api.setAppearance).not.toHaveBeenCalled();
  });

  it("closing the picker with the change button drops an unapplied preview", async () => {
    render(<Appearance />);
    const input = await openDark();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(bg()).toBe("#010203");
    fireEvent.click(changeButtons()[0]);
    expect(screen.queryByRole("combobox", { name: "Filter themes" })).toBeNull();
    expect(bg()).toBe("#0a0b0c");
    expect(api.setAppearance).not.toHaveBeenCalled();
  });

  it("unmounting mid-preview restores the saved colours", async () => {
    const { unmount } = render(<Appearance />);
    fireEvent.keyDown(await openDark(), { key: "ArrowDown" });
    expect(bg()).toBe("#010203");
    unmount();
    expect(bg()).toBe("#0a0b0c");
  });

  it("mousedown on an option is default-prevented so the filter keeps focus", async () => {
    render(<Appearance />);
    await openDark();
    const opt = screen.getByRole("option", { name: /Harbor/ });
    const down = createEvent.mouseDown(opt);
    fireEvent(opt, down);
    expect(down.defaultPrevented).toBe(true);
  });

  it("clicking an option previews it without saving", async () => {
    render(<Appearance />);
    await openDark();
    const harbor = screen.getByRole("option", { name: /Harbor/ });
    fireEvent.click(harbor);
    expect(harbor.getAttribute("aria-selected")).toBe("true");
    expect(bg()).toBe("#010203");
    expect(api.setAppearance).not.toHaveBeenCalled();
  });

  it("renders a count on every chip", async () => {
    render(<Appearance />);
    await openDark();
    for (const name of ["all 4", "dark 2", "light 2", "flick 2", "ide ports 1", "imported 1"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
  });

  it("a chip filters the list", async () => {
    render(<Appearance />);
    await openDark();
    fireEvent.click(screen.getByRole("button", { name: "all 4" }));
    expect(optionNames()).toEqual(["Night", "Harbor", "Paper", "Sage"]);
    fireEvent.click(screen.getByRole("button", { name: "ide ports 1" }));
    expect(optionNames()).toEqual(["Harbor"]);
    expect(screen.getByRole("button", { name: "ide ports 1" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
  });

  it("renders the ratios of the selected theme and follows the selection", async () => {
    render(<Appearance />);
    const input = await openDark();
    expect(screen.getByText("12.3:1")).toBeTruthy();
    expect(screen.getByText("8.3:1")).toBeTruthy();
    expect(screen.getByText("7.3:1")).toBeTruthy();
    expect(screen.getByText("9.3:1")).toBeTruthy();
    expect(screen.queryByText(/raised for contrast/)).toBeNull();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByText("15.8:1")).toBeTruthy();
    expect(screen.getByText("raised for contrast: muted")).toBeTruthy();
    expect(screen.getByText("from github.com/example/harbor")).toBeTruthy();
  });

  it("the import button is disabled and says why", async () => {
    render(<Appearance />);
    await openDark();
    const btn = screen.getByRole("button", { name: /import VS Code theme/ });
    expect(btn.hasAttribute("disabled")).toBe(true);
    const reason = screen.getByText("Importing arrives in a later update.");
    expect(btn.getAttribute("aria-describedby")).toBe(reason.id);
  });
});
