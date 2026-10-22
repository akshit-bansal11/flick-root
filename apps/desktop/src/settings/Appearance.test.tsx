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

  it("saves a scheme pick and moves the segmented lens to it", async () => {
    render(<Appearance />);
    const group = await screen.findByRole("radiogroup", { name: "Colour scheme" });
    expect(group.style.getPropertyValue("--n")).toBe("3");
    expect(group.style.getPropertyValue("--i")).toBe("0");
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(api.setAppearance).toHaveBeenCalledWith({ scheme: "dark" }));
    await waitFor(() => expect(group.style.getPropertyValue("--i")).toBe("1"));
  });

  it("shows the slider value in an output and drives the track fill", async () => {
    render(<Appearance />);
    const slider = await screen.findByRole("slider");
    expect(screen.getByRole("status").textContent).toBe("90%");
    expect(Number.parseFloat(slider.style.getPropertyValue("--val"))).toBeCloseTo(66.67, 1);
    fireEvent.change(slider, { target: { value: "95" } });
    expect(screen.getByRole("status").textContent).toBe("95%");
    expect(slider.style.getPropertyValue("--val")).toBe("100%");
  });

  it("renders a decorative live preview", async () => {
    render(<Appearance />);
    await screen.findByRole("slider");
    const pv = document.querySelector(".s-pv");
    expect(pv?.getAttribute("aria-hidden")).toBe("true");
    expect(pv?.textContent).toContain("2.5 kg to lb");
  });

  it("disables the scheme radios for a theme that forces its scheme", async () => {
    render(<Appearance />);
    fireEvent.click(await screen.findByRole("radio", { name: "Midnight" }));
    await waitFor(() =>
      expect((screen.getByRole("radio", { name: "Dark" }) as HTMLInputElement).disabled).toBe(true),
    );
  });

  it("shows the error and reverts when saving fails", async () => {
    vi.mocked(api.setAppearance).mockRejectedValue({ code: "invalid_appearance", message: "nope" });
    render(<Appearance />);
    fireEvent.click(await screen.findByRole("radio", { name: "Ember" }));
    expect((await screen.findByRole("alert")).textContent).toBe("nope");
    expect((screen.getByRole("radio", { name: "Glass" }) as HTMLInputElement).checked).toBe(true);
  });
});
