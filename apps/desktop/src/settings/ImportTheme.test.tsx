import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ThemeView } from "../theme";
import * as api from "./api";
import { ImportTheme } from "./ImportTheme";
import { type ImportPreview, MAX_THEME_BYTES } from "./types";

vi.mock("./api", () => ({
  previewVscodeTheme: vi.fn(),
  saveImportedTheme: vi.fn(),
}));

const PREVIEW: ImportPreview = {
  token: "tok-1",
  id: "imp-sage",
  name: "Sage Night",
  rows: [
    { role: "text", key: "editor.foreground", hex: "#ffffff", used: true },
    { role: "muted", key: "descriptionForeground", hex: "#777777", used: true },
    { role: "accent", key: "focusBorder", hex: "#3399ff", used: false },
  ],
  adjusted: ["muted 2.1->4.5"],
  notes: ["Ignored 3 token colours."],
};

const LIST: ThemeView[] = [];

/** A theme file; jsdom may lack File.text(), so it is stubbed on the instance. */
function themeFile(name = "sage.json", text = "{}") {
  const file = new File([text], name, { type: "application/json" });
  Object.defineProperty(file, "text", { value: () => Promise.resolve(text) });
  return file;
}

function pick(file: File) {
  const input = screen.getByTestId("import-file");
  Object.defineProperty(input, "files", { value: [file], configurable: true });
  fireEvent.change(input);
}

const importBtn = () => screen.getByRole("button", { name: "import VS Code theme…" });
const cancelBtn = () => screen.getByRole("button", { name: "cancel" });
const confirmBtn = () => screen.getByRole("button", { name: 'import as "Sage Night"' });

async function openDialog(onImported = vi.fn()) {
  render(<ImportTheme onImported={onImported} />);
  pick(themeFile());
  await screen.findByRole("dialog");
  return onImported;
}

beforeEach(() => {
  vi.mocked(api.previewVscodeTheme).mockResolvedValue(PREVIEW);
  vi.mocked(api.saveImportedTheme).mockResolvedValue(LIST);
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ImportTheme", () => {
  it("rejects an oversize file with an alert and never previews it", async () => {
    render(<ImportTheme onImported={vi.fn()} />);
    const file = themeFile();
    Object.defineProperty(file, "size", { value: MAX_THEME_BYTES + 1 });
    pick(file);
    expect((await screen.findByRole("alert")).textContent).toContain("larger than 1 MB");
    expect(api.previewVscodeTheme).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("previews the file text and name, then shows the mapping table", async () => {
    render(<ImportTheme onImported={vi.fn()} />);
    pick(themeFile("sage.json", '{"a":1}'));
    const dialog = await screen.findByRole("dialog", { name: "Import sage.json" });
    expect(api.previewVscodeTheme).toHaveBeenCalledWith('{"a":1}', "sage.json");
    expect(dialog.querySelectorAll("tbody tr")).toHaveLength(3);
    expect(screen.getByText("used as is")).toBeTruthy();
    expect(screen.getByText("raised to 4.5:1")).toBeTruthy();
    expect(screen.getByText("fallback")).toBeTruthy();
    expect(screen.getByText("editor.foreground")).toBeTruthy();
    expect(screen.getByText("Ignored 3 token colours.")).toBeTruthy();
    expect(screen.getByText(/Missing keys fall back to related colours/)).toBeTruthy();
    expect(document.activeElement).toBe(cancelBtn());
  });

  it("cancel closes, returns focus to the import button and saves nothing", async () => {
    await openDialog();
    fireEvent.click(cancelBtn());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(importBtn());
    expect(api.saveImportedTheme).not.toHaveBeenCalled();
  });

  it("Escape closes, returns focus to the import button and saves nothing", async () => {
    await openDialog();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(importBtn());
    expect(api.saveImportedTheme).not.toHaveBeenCalled();
  });

  it("confirm saves the token and hands back the list and the new id", async () => {
    const onImported = await openDialog();
    fireEvent.click(confirmBtn());
    await waitFor(() => expect(onImported).toHaveBeenCalledWith(LIST, "imp-sage"));
    expect(api.saveImportedTheme).toHaveBeenCalledWith("tok-1");
    expect(api.saveImportedTheme).toHaveBeenCalledTimes(1);
  });

  it("a preview error shows under the button and opens no dialog", async () => {
    vi.mocked(api.previewVscodeTheme).mockRejectedValue({ code: "bad", message: "Not a theme" });
    render(<ImportTheme onImported={vi.fn()} />);
    pick(themeFile());
    expect((await screen.findByRole("alert")).textContent).toBe("Not a theme");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("a save error stays inside the open dialog and does not call onImported", async () => {
    vi.mocked(api.saveImportedTheme).mockRejectedValue({ code: "io", message: "Disk full" });
    const onImported = await openDialog();
    fireEvent.click(confirmBtn());
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Disk full");
    expect(screen.getByRole("dialog").contains(alert)).toBe(true);
    expect(onImported).not.toHaveBeenCalled();
    await waitFor(() => expect(confirmBtn().hasAttribute("disabled")).toBe(false));
  });

  it("picking the same file twice previews twice", async () => {
    render(<ImportTheme onImported={vi.fn()} />);
    const file = themeFile();
    pick(file);
    await screen.findByRole("dialog");
    fireEvent.click(cancelBtn());
    pick(file);
    await screen.findByRole("dialog");
    expect(api.previewVscodeTheme).toHaveBeenCalledTimes(2);
  });
});
