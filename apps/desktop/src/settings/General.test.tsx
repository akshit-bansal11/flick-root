import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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

const onOff = (label: string, state: "on" | "off") =>
  within(screen.getByRole("group", { name: label })).getByRole("button", { name: state });
const skipEdit = () => screen.getByRole("button", { name: "Edit excluded folders" });
const openSkipEditor = () => fireEvent.click(skipEdit());

describe("General hotkey capture", () => {
  afterEach(cleanup);
  it("rejects a bare key without calling Rust", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("change"));
    fireEvent.keyDown(screen.getByText(/Press keys/), { key: "Tab", code: "Tab" });
    expect(screen.getByRole("alert").textContent).toBe(MSG_NO_MODIFIER);
    expect(api.setGeneral).not.toHaveBeenCalled();
  });

  it("sends a valid combo", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("change"));
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
    fireEvent.click(onOff("index all drives", "off"));
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
    fireEvent.click(screen.getByText("take the tour"));
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
    openSkipEditor();
    expect(screen.getByLabelText("Remove D:\\Games")).toBeTruthy();
    expect(screen.getByLabelText("Remove node_modules")).toBeTruthy();
    expect(screen.queryByLabelText("Remove F:\\")).toBeNull();
  });

  it("the edit button toggles the excluded-folders editor", () => {
    render(<General settings={settings} onChange={() => {}} />);
    const edit = skipEdit();
    expect(edit.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByLabelText("Exclude")).toBeNull();
    fireEvent.click(edit);
    expect(edit.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByLabelText("Exclude")).toBeTruthy();
    fireEvent.click(edit);
    expect(edit.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByLabelText("Exclude")).toBeNull();
  });

  it("unchecking a drive adds it to the skip list", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByLabelText(/E:\\ · removable/));
    expect(api.setProgramSkip).toHaveBeenCalledWith([...settings.programSkip, "E:\\"]);
  });

  it("removes an entry", () => {
    render(<General settings={settings} onChange={() => {}} />);
    openSkipEditor();
    fireEvent.click(screen.getByLabelText("Remove D:\\Games"));
    expect(api.setProgramSkip).toHaveBeenCalledWith(["F:\\", "node_modules"]);
  });

  it("adds an entry on Enter", () => {
    render(<General settings={settings} onChange={() => {}} />);
    openSkipEditor();
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
    openSkipEditor();
    const input = screen.getByLabelText("Exclude");
    fireEvent.change(input, { target: { value: "[" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect((await screen.findByRole("alert")).textContent).toBe("bad pattern");
    expect(onChange).toHaveBeenLastCalledWith(settings);
  });
});

describe("General switches", () => {
  afterEach(cleanup);
  it("uses the one OnOff component for every switch", () => {
    render(<General settings={settings} onChange={() => {}} />);
    expect(screen.getByRole("group", { name: "launch at startup" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "check for updates" })).toBeTruthy();
    expect(document.querySelectorAll('[role="switch"]')).toHaveLength(0);
  });
});

describe("General history", () => {
  afterEach(cleanup);
  it("cancel returns to clear without clearing", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("clear"));
    expect(screen.getByText("confirm clear")).toBeTruthy();
    fireEvent.click(screen.getByText("cancel"));
    expect(screen.getByText("clear")).toBeTruthy();
    expect(screen.queryByText("confirm clear")).toBeNull();
    expect(api.clearHistory).not.toHaveBeenCalled();
  });

  it("confirm clear clears once and says so", async () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(screen.getByText("clear"));
    fireEvent.click(screen.getByText("confirm clear"));
    expect(await screen.findByText("history cleared")).toBeTruthy();
    expect(api.clearHistory).toHaveBeenCalledTimes(1);
    expect(api.clearHistory).toHaveBeenCalledWith("all");
    expect(screen.getByText("clear")).toBeTruthy();
  });
});

describe("programs scan toggle", () => {
  afterEach(cleanup);
  it("sends scanPrograms", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(onOff("find programs on all drives", "off"));
    expect(api.setGeneral).toHaveBeenCalledWith({ scanPrograms: false });
  });
});

describe("General focus and help", () => {
  afterEach(cleanup);
  const btn = (name: string) => screen.getByRole("button", { name });

  it("focus follows clear to confirm clear and back to clear on cancel", () => {
    render(<General settings={settings} onChange={() => {}} />);
    fireEvent.click(btn("clear"));
    expect(document.activeElement).toBe(btn("confirm clear"));
    fireEvent.click(btn("cancel"));
    expect(document.activeElement).toBe(btn("clear"));
  });

  it("removing a folder focuses the next remove button, the last one the add input", () => {
    render(<General settings={settings} onChange={() => {}} />);
    openSkipEditor();
    fireEvent.click(screen.getByLabelText("Remove D:\Games"));
    expect(document.activeElement).toBe(screen.getByLabelText("Remove node_modules"));
    fireEvent.click(screen.getByLabelText("Remove node_modules"));
    expect(document.activeElement).toBe(screen.getByLabelText("Exclude"));
  });

  it("shows the help sentences under the rows without hover", () => {
    render(<General settings={settings} onChange={() => {}} />);
    for (const text of [
      /Once a day, asks GitHub/,
      /System folders like Windows and Program Files are skipped/,
      /Scans every fixed and removable drive.* 42 found\./,
      /Untick a drive and the program scan skips it/,
    ]) {
      expect(screen.getByText(text).className).toContain("s-help");
    }
  });
});
