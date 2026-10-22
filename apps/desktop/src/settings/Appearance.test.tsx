import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Appearance } from "./Appearance";
import * as api from "./api";

vi.mock("./api", () => ({ getAppearance: vi.fn(), setAppearance: vi.fn() }));

const cur = { theme: "glass", glass: 90, scheme: "system" } as const;

beforeEach(() => {
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  vi.mocked(api.getAppearance).mockResolvedValue({ ...cur });
  vi.mocked(api.setAppearance).mockImplementation((p) => Promise.resolve({ ...cur, ...p }));
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("Appearance", () => {
  it("lists themes as radios and saves a pick", async () => {
    render(<Appearance />);
    fireEvent.click(await screen.findByRole("radio", { name: "Midnight" }));
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ theme: "midnight" }));
    expect(document.documentElement.dataset.theme).toBe("midnight");
  });

  it("saves the glass slider and disables it for solid", async () => {
    render(<Appearance />);
    const slider = await screen.findByRole("slider");
    fireEvent.change(slider, { target: { value: "85" } });
    fireEvent.change(slider, { target: { value: "92" } });
    expect(api.setAppearance).not.toHaveBeenCalled();
    fireEvent.pointerUp(slider);
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ glass: 92 }));
    expect(api.setAppearance).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("radio", { name: "Solid" }));
    await waitFor(() => expect((slider as HTMLInputElement).disabled).toBe(true));
  });

  it("shows the error and reverts when saving fails", async () => {
    vi.mocked(api.setAppearance).mockRejectedValue({ code: "invalid_appearance", message: "nope" });
    render(<Appearance />);
    fireEvent.click(await screen.findByRole("radio", { name: "Ember" }));
    expect((await screen.findByRole("alert")).textContent).toBe("nope");
    expect((screen.getByRole("radio", { name: "Glass" }) as HTMLInputElement).checked).toBe(true);
  });
});
