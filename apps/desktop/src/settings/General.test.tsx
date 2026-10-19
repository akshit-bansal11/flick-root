import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MSG_NO_MODIFIER } from "../firstrun/hotkeyRules";
import * as api from "./api";
import { General } from "./General";
import type { SettingsView } from "./types";

vi.mock("./api", () => ({
  setGeneral: vi.fn(() => Promise.resolve({})),
  setRoots: vi.fn(),
  setIncludeDrives: vi.fn(),
  clearHistory: vi.fn(),
  reindex: vi.fn(),
}));

const settings: SettingsView = {
  hotkeyLauncher: "Alt+Space",
  hotkeyExplain: "Ctrl+Alt+E",
  theme: "system",
  launchAtStartup: false,
  findEnabled: false,
  queryEnabled: false,
  roots: [],
  extraRoots: [],
  includeDrives: true,
};

describe("General hotkey capture", () => {
  afterEach(cleanup);
  it("rejects a bare key without calling Rust", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("Change"));
    fireEvent.keyDown(screen.getByText(/Press keys/), { key: "Tab", code: "Tab" });
    expect(screen.getByRole("alert").textContent).toBe(MSG_NO_MODIFIER);
    expect(api.setGeneral).not.toHaveBeenCalled();
  });

  it("sends a valid combo", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("Change"));
    fireEvent.keyDown(screen.getByText(/Press keys/), {
      key: "k",
      code: "KeyK",
      ctrlKey: true,
    });
    expect(api.setGeneral).toHaveBeenCalledWith({ hotkeyLauncher: "Ctrl+K" });
  });
});

describe("General index all drives", () => {
  afterEach(cleanup);
  it("flips optimistically, then reverts with an error when Rust fails", async () => {
    vi.mocked(api.setIncludeDrives).mockRejectedValueOnce({ code: "x", message: "scan failed" });
    const onChange = vi.fn();
    render(<General settings={settings} onChange={onChange} />);
    fireEvent.click(screen.getByRole("switch", { name: "Index all drives" }));
    expect(api.setIncludeDrives).toHaveBeenCalledWith(false);
    expect(onChange).toHaveBeenNthCalledWith(1, { ...settings, includeDrives: false });
    expect((await screen.findByRole("alert")).textContent).toBe("scan failed");
    expect(onChange).toHaveBeenLastCalledWith(settings);
  });
});
