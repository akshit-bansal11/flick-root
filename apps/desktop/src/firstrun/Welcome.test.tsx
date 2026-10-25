import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MODES } from "../launcher/reducer";
import { Welcome } from "./Welcome";

afterEach(cleanup);
const fns = () => ({ onTour: vi.fn(), onSkip: vi.fn(), onContinue: vi.fn() });

describe("Welcome", () => {
  it("shows the question, chips, and focuses Take the tour", () => {
    const f = fns();
    render(<Welcome note={false} {...f} />);
    expect(screen.getByText("Take a 2-minute interactive tour?")).toBeTruthy();
    const chips = [...document.querySelectorAll(".fr-chip")];
    expect(chips).toHaveLength(5);
    for (const [i, m] of Object.values(MODES).entries()) {
      expect(chips[i]?.querySelector("kbd")?.textContent).toBe(m.prefix);
    }
    expect(document.activeElement).toBe(screen.getByText("take the tour"));
    fireEvent.click(screen.getByText("take the tour"));
    fireEvent.click(screen.getByText("not now"));
    expect(f.onTour).toHaveBeenCalled();
    expect(f.onSkip).toHaveBeenCalled();
  });

  it("note focuses Continue", () => {
    const f = fns();
    render(<Welcome note {...f} />);
    expect(screen.getByText(/No problem/).textContent).toContain(
      "Settings > General > Take the tour",
    );
    expect(document.activeElement).toBe(screen.getByText("continue"));
    fireEvent.click(screen.getByText("continue"));
    expect(f.onContinue).toHaveBeenCalled();
  });
});
