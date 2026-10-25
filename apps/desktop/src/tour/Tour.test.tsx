import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../settings/api";
import { ROLES, type ThemeRoles, type ThemeView } from "../theme";
import { Tour } from "./Tour";

const roles = Object.fromEntries(ROLES.map((r) => [r, "#123456"])) as ThemeRoles;
const view = (id: string, name: string, dark: boolean, bg: string): ThemeView => ({
  id,
  name,
  source: "builtin",
  dark,
  roles: { ...roles, bg },
  ratios: { text: 9, muted: 5, accentText: 5, onAccent: 5, selText: 9, mutedBg: 5, mutedSel: 5 },
  adjusted: [],
  upstream: null,
});
const THEMES = [
  view("flick-default", "Flick Default", true, "#010101"),
  view("flick-default-light", "Flick Default Light", false, "#020202"),
  view("frost", "Frost", false, "#030303"),
  view("midnight", "Midnight", true, "#040404"),
  view("aurora", "Aurora", true, "#050505"),
  view("ember", "Ember", true, "#060606"),
];

vi.mock("../settings/api", () => ({
  setHotkeysPaused: vi.fn(() => Promise.resolve()),
  setTutorialDone: vi.fn(() => Promise.resolve()),
  setAppearance: vi.fn(() => Promise.resolve({})),
  listThemes: vi.fn(() => Promise.resolve(THEMES)),
  getSettings: vi.fn(() =>
    Promise.resolve({ hotkeyLauncher: "Alt+Space", hotkeyExplain: "Alt+Shift+Space" }),
  ),
}));

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.clearAllMocks();
});

const mount = async () => {
  const onDone = vi.fn();
  const view = render(<Tour onDone={onDone} />);
  await act(async () => {});
  return { onDone, ...view };
};
const title = () => screen.getByRole("heading", { level: 2 }).textContent;
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const wait = (ms: number) =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });

describe("Tour", () => {
  it("pauses the global hotkeys on mount and resumes on unmount", async () => {
    const { unmount } = await mount();
    expect(api.setHotkeysPaused).toHaveBeenLastCalledWith(true);
    unmount();
    expect(api.setHotkeysPaused).toHaveBeenLastCalledWith(false);
  });

  it("resumes the hotkeys on beforeunload", async () => {
    await mount();
    window.dispatchEvent(new Event("beforeunload"));
    expect(api.setHotkeysPaused).toHaveBeenLastCalledWith(false);
  });

  it("moves on by itself after the hotkey is pressed", async () => {
    await mount();
    expect(title()).toBe("Open the bar");
    fireEvent.keyDown(window, { key: " ", altKey: true });
    await wait(700);
    expect(title()).toBe("Type anything");
  });

  it("walks with Next, Back and PageDown/PageUp", async () => {
    await mount();
    click("next");
    expect(title()).toBe("Type anything");
    fireEvent.keyDown(window, { key: "PageDown" });
    expect(title()).toBe("/ Open apps and files");
    fireEvent.keyDown(window, { key: "PageUp" });
    click("back");
    expect(title()).toBe("Open the bar");
  });

  it("Show me types into the real launcher and Enter completes the Open step", async () => {
    await mount();
    click("next");
    click("next");
    click("show me");
    await wait(45 * 12);
    const input = screen.getByRole("combobox");
    expect((input as HTMLInputElement).value).toBe("spotify");
    expect(screen.getByRole("option", { name: /Spotify/ })).toBeTruthy();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByText("Would open Spotify")).toBeTruthy();
    expect(screen.getByRole("img", { name: "Done" })).toBeTruthy();
    await wait(700);
    expect(title()).toBe("> Commands");
  });

  it("Esc inside the demo never ends the tour", async () => {
    const { onDone } = await mount();
    click("next");
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
    expect(onDone).not.toHaveBeenCalled();
    expect(title()).toBe("Type anything");
  });

  it("Show me types a plain search and Enter completes the Type anything step", async () => {
    await mount();
    click("next");
    click("show me");
    await wait(45 * 5);
    const input = screen.getByRole("combobox");
    expect((input as HTMLInputElement).value).toBe("spo");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByRole("img", { name: "Done" })).toBeTruthy();
    await wait(700);
    expect(title()).toBe("/ Open apps and files");
  });

  it("Skip ends without marking the tour done", async () => {
    const { onDone } = await mount();
    click("Skip tour");
    await act(async () => {});
    expect(onDone).toHaveBeenCalledWith(false);
    expect(api.setTutorialDone).not.toHaveBeenCalled();
  });

  it("Finish marks the tutorial done", async () => {
    const { onDone } = await mount();
    for (let i = 0; i < 10; i++) click("next");
    expect(title()).toBe("You are set");
    click("finish");
    await act(async () => {});
    expect(api.setTutorialDone).toHaveBeenCalledWith(true);
    expect(onDone).toHaveBeenCalledWith(true);
  });

  const toThemes = async () => {
    await mount();
    for (let i = 0; i < 9; i++) click("next");
  };
  const demoBg = () => screen.getByTestId("demo").style.getPropertyValue("--l-bg");

  it("renders one swatch per tour theme from list_themes", async () => {
    await toThemes();
    const names = screen.getAllByRole("button", { pressed: false }).map((b) => b.textContent);
    expect(names).toEqual(["Flick Default Light", "Frost", "Midnight", "Aurora", "Ember"]);
    expect(screen.getByRole("button", { name: "Flick Default", pressed: true })).toBeTruthy();
  });

  it("a theme swatch restyles the demo", async () => {
    await toThemes();
    expect(demoBg()).toBe("#010101");
    click("Midnight");
    expect(demoBg()).toBe("#040404");
  });

  it("Use this theme saves a dark theme as darkTheme", async () => {
    await toThemes();
    click("Midnight");
    click("Use this theme");
    await act(async () => {});
    expect(api.setAppearance).toHaveBeenCalledWith({ darkTheme: "midnight" });
  });

  it("Use this theme saves a light theme as lightTheme", async () => {
    await toThemes();
    click("Frost");
    click("Use this theme");
    await act(async () => {});
    expect(api.setAppearance).toHaveBeenCalledWith({ lightTheme: "frost" });
  });

  it("Next theme cycles through the tour themes and wraps", async () => {
    await toThemes();
    click("next theme");
    expect(demoBg()).toBe("#020202");
    for (let i = 0; i < 5; i++) click("next theme");
    expect(demoBg()).toBe("#010101");
  });
});
