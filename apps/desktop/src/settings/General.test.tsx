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
  setProgramSkip: vi.fn(() => Promise.resolve({})),
  clearHistory: vi.fn(),
  reindex: vi.fn(),
}));

const settings: SettingsView = {
  hotkeyLauncher: "Alt+Space",
  hotkeyExplain: "Ctrl+Alt+E",
  launchAtStartup: false,
  checkUpdates: true,
  findEnabled: false,
  queryEnabled: false,
  roots: [],
  extraRoots: [],
  includeDrives: true,
  scanPrograms: true,
  programSkip: ["F:\\", "D:\\Games", "node_modules"],
  drives: [
    { root: "C:\\", kind: "fixed", present: true },
    { root: "E:\\", kind: "removable", present: true },
    { root: "F:\\", kind: "removable", present: false },
  ],
  programsCount: 42,
  programsCapped: false,
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

describe("General take the tour", () => {
  afterEach(cleanup);
  it("asks the window to open the tour", () => {
    const onTour = vi.fn();
    render(<General settings={settings} onChange={() => {}} onTour={onTour} />);
    fireEvent.click(screen.getByText("Take the tour"));
    expect(onTour).toHaveBeenCalled();
  });
});

describe("program exclusions", () => {
  afterEach(cleanup);
  it("renders drives and excluded entries", () => {
    render(<General settings={settings} onChange={() => {}} />);
    expect(screen.getByLabelText(/C:\\ · fixed/)).toHaveProperty("checked", true);
    expect(screen.getByLabelText(/F:\\ · removable \(not connected\)/)).toHaveProperty(
      "checked",
      false,
    );
    expect(screen.getByLabelText("Remove D:\\Games")).toBeTruthy();
    expect(screen.getByLabelText("Remove node_modules")).toBeTruthy();
    expect(screen.queryByLabelText("Remove F:\\")).toBeNull();
  });

  it("unchecking a drive adds it to the skip list", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByLabelText(/E:\\ · removable/));
    expect(api.setProgramSkip).toHaveBeenCalledWith([...settings.programSkip, "E:\\"]);
  });

  it("removes an entry", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByLabelText("Remove D:\\Games"));
    expect(api.setProgramSkip).toHaveBeenCalledWith(["F:\\", "node_modules"]);
  });

  it("adds an entry on Enter", () => {
    render(<General settings={settings} onChange={() => {}} />);
    const input = screen.getByLabelText("Exclude");
    fireEvent.change(input, { target: { value: ".*" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(api.setProgramSkip).toHaveBeenCalledWith([...settings.programSkip, ".*"]);
  });

  it("shows invalid_skip as an alert and rolls back", async () => {
    vi.mocked(api.setProgramSkip).mockRejectedValueOnce({
      code: "invalid_skip",
      message: "bad pattern",
    });
    const onChange = vi.fn();
    render(<General settings={settings} onChange={onChange} />);
    const input = screen.getByLabelText("Exclude");
    fireEvent.change(input, { target: { value: "[" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect((await screen.findByRole("alert")).textContent).toBe("bad pattern");
    expect(onChange).toHaveBeenLastCalledWith(settings);
  });
});

describe("General switches", () => {
  afterEach(cleanup);
  it("uses the one Toggle component for every switch", () => {
    render(<General settings={settings} onChange={() => {}} />);
    expect(screen.getByRole("switch", { name: "Launch at startup" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Check for updates" })).toBeTruthy();
    expect(document.querySelectorAll('button[role="switch"]:not(.s-switch)')).toHaveLength(0);
  });
});

describe("programs scan toggle", () => {
  afterEach(cleanup);
  it("sends scanPrograms", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByRole("switch", { name: "Find programs on all drives" }));
    expect(api.setGeneral).toHaveBeenCalledWith({ scanPrograms: false });
  });
});
