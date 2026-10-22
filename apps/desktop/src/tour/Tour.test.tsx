import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../settings/api";
import { Tour } from "./Tour";

vi.mock("../settings/api", () => ({
  setHotkeysPaused: vi.fn(() => Promise.resolve()),
  setTutorialDone: vi.fn(() => Promise.resolve()),
  setAppearance: vi.fn(() => Promise.resolve({})),
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
    expect(title()).toBe("/ Open apps and files");
  });

  it("walks with Next, Back and PageDown/PageUp", async () => {
    await mount();
    click("Next");
    expect(title()).toBe("/ Open apps and files");
    fireEvent.keyDown(window, { key: "PageDown" });
    expect(title()).toBe("> Commands");
    fireEvent.keyDown(window, { key: "PageUp" });
    click("Back");
    expect(title()).toBe("Open the bar");
  });

  it("Show me types into the real launcher and Enter completes the Open step", async () => {
    await mount();
    click("Next");
    click("Show me");
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
    click("Next");
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
    expect(onDone).not.toHaveBeenCalled();
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
    for (let i = 0; i < 9; i++) click("Next");
    expect(title()).toBe("You are set");
    click("Finish");
    await act(async () => {});
    expect(api.setTutorialDone).toHaveBeenCalledWith(true);
    expect(onDone).toHaveBeenCalledWith(true);
  });

  it("a theme swatch restyles the demo and Use this theme saves it", async () => {
    await mount();
    for (let i = 0; i < 8; i++) click("Next");
    click("midnight");
    expect(screen.getByTestId("demo").dataset.theme).toBe("midnight");
    click("Use this theme");
    await act(async () => {});
    expect(api.setAppearance).toHaveBeenCalledWith({ theme: "midnight" });
  });
});
