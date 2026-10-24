import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ROLES, type ThemeRoles, type ThemeView } from "../theme";
import { Appearance } from "./Appearance";
import * as api from "./api";
import type { AppearanceView } from "./types";

vi.mock("./api", () => ({
  getAppearance: vi.fn(),
  setAppearance: vi.fn(),
  listThemes: vi.fn(),
}));

const roles = Object.fromEntries(ROLES.map((r) => [r, "#123456"])) as ThemeRoles;
const ratios = {
  text: 9,
  muted: 5,
  accentText: 5,
  onAccent: 5,
  selText: 9,
  mutedBg: 5,
  mutedSel: 5,
};
const theme = (id: string, name: string, dark: boolean): ThemeView => ({
  id,
  name,
  source: "builtin",
  dark,
  roles,
  ratios,
  adjusted: [],
  upstream: null,
});
const THEMES = [
  theme("d1", "Night", true),
  theme("d2", "Harbor", true),
  theme("l1", "Paper", false),
  theme("l2", "Sage", false),
];
const cur: AppearanceView = {
  scheme: "system",
  darkTheme: "d1",
  lightTheme: "l1",
  active: null,
};
const active = { id: "d2", dark: true, roles: { ...roles, bg: "#010203" } };

beforeEach(() => {
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  vi.mocked(api.getAppearance).mockResolvedValue({ ...cur });
  vi.mocked(api.listThemes).mockResolvedValue(THEMES);
  vi.mocked(api.setAppearance).mockImplementation((p) => Promise.resolve({ ...cur, ...p }));
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("Appearance", () => {
  it("fills each theme select from list_themes, filtered by scheme", async () => {
    render(<Appearance />);
    const dark = await screen.findByLabelText("Dark theme");
    const light = screen.getByLabelText("Light theme");
    const names = (el: HTMLElement) => [...el.querySelectorAll("option")].map((o) => o.textContent);
    expect(names(dark)).toEqual(["Night", "Harbor"]);
    expect(names(light)).toEqual(["Paper", "Sage"]);
    expect((dark as HTMLSelectElement).value).toBe("d1");
    expect((light as HTMLSelectElement).value).toBe("l1");
  });

  it("saves the dark theme id and applies the returned theme at once", async () => {
    vi.mocked(api.setAppearance).mockResolvedValue({ ...cur, darkTheme: "d2", active });
    render(<Appearance />);
    fireEvent.change(await screen.findByLabelText("Dark theme"), { target: { value: "d2" } });
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ darkTheme: "d2" }));
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--l-bg")).toBe("#010203"),
    );
  });

  it("saves the light theme id", async () => {
    render(<Appearance />);
    fireEvent.change(await screen.findByLabelText("Light theme"), { target: { value: "l2" } });
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ lightTheme: "l2" }));
  });

  it("saves a scheme pick and moves the segmented lens to it", async () => {
    render(<Appearance />);
    const group = await screen.findByRole("radiogroup", { name: "Colour scheme" });
    expect(group.style.getPropertyValue("--n")).toBe("3");
    expect(group.style.getPropertyValue("--i")).toBe("0");
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ scheme: "dark" }));
    await waitFor(() => expect(group.style.getPropertyValue("--i")).toBe("1"));
  });

  it("shows the error and reverts when saving fails", async () => {
    vi.mocked(api.setAppearance).mockRejectedValue({ code: "invalid_appearance", message: "nope" });
    render(<Appearance />);
    fireEvent.click(await screen.findByRole("radio", { name: "Dark" }));
    expect((await screen.findByRole("alert")).textContent).toBe("nope");
    expect((screen.getByRole("radio", { name: "System" }) as HTMLInputElement).checked).toBe(true);
  });
});
