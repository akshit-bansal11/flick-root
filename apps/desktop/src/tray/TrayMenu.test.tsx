import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { APP_DISPLAY_NAME } from "../brand";
import type { TrayMenuEvent } from "../ipc";
import { TrayMenu } from "./TrayMenu";

const base: TrayMenuEvent = { appVersion: "0.0.1", hotkey: "Alt+Space", update: null };

const setup = (event: TrayMenuEvent = base) => {
  const onRun = vi.fn();
  const onClose = vi.fn();
  render(<TrayMenu event={event} onRun={onRun} onClose={onClose} />);
  return { onRun, onClose };
};

afterEach(() => vi.restoreAllMocks());

describe("TrayMenu", () => {
  it("exposes menu, four menuitems and one separator", () => {
    setup();
    expect(screen.getByRole("menu", { name: APP_DISPLAY_NAME })).toBeTruthy();
    expect(screen.getAllByRole("menuitem")).toHaveLength(4);
    expect(screen.getAllByRole("separator")).toHaveLength(1);
  });

  it("shows the name, version and the hotkey caps on Open", () => {
    setup();
    expect(screen.getByText("v0.0.1")).toBeTruthy();
    const open = screen.getByRole("menuitem", { name: /^Open/ });
    expect([...open.querySelectorAll("kbd")].map((k) => k.textContent)).toEqual(["Alt", "Space"]);
  });

  it("uses roving tabindex starting on Open", () => {
    setup();
    const tabs = screen.getAllByRole("menuitem").map((i) => i.getAttribute("tabindex"));
    expect(tabs).toEqual(["0", "-1", "-1", "-1"]);
  });

  it("ArrowDown then Enter runs settings", () => {
    const { onRun } = setup();
    fireEvent.keyDown(window, { key: "ArrowDown" });
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onRun).toHaveBeenCalledWith("settings");
  });

  it("End then Space runs quit", () => {
    const { onRun } = setup();
    fireEvent.keyDown(window, { key: "End" });
    fireEvent.keyDown(window, { key: " " });
    expect(onRun).toHaveBeenCalledWith("quit");
  });

  it("Escape closes", () => {
    const { onClose } = setup();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("hover selects and click runs", () => {
    const { onRun } = setup();
    const quit = screen.getByRole("menuitem", { name: "Quit" });
    fireEvent.mouseEnter(quit);
    expect(quit.getAttribute("tabindex")).toBe("0");
    fireEvent.click(quit);
    expect(onRun).toHaveBeenCalledWith("quit");
  });

  it("labels the update item with the version", () => {
    setup({ ...base, update: { version: "0.0.2" } });
    expect(screen.getByRole("menuitem", { name: "Install 0.0.2" })).toBeTruthy();
  });
});
