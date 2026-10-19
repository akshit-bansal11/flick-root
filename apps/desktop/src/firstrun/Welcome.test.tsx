import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Welcome } from "./Welcome";

afterEach(cleanup);
const fns = () => ({ onTour: vi.fn(), onSkip: vi.fn(), onContinue: vi.fn() });

describe("Welcome", () => {
  it("shows the question, chips, and focuses Take the tour", () => {
    const f = fns();
    render(<Welcome note={false} {...f} />);
    expect(screen.getByText("Take a 2-minute interactive tour?")).toBeTruthy();
    expect(document.querySelectorAll(".fr-chip")).toHaveLength(4);
    expect(document.activeElement).toBe(screen.getByText("Take the tour"));
    fireEvent.click(screen.getByText("Take the tour"));
    fireEvent.click(screen.getByText("Not now"));
    expect(f.onTour).toHaveBeenCalled();
    expect(f.onSkip).toHaveBeenCalled();
  });

  it("note focuses Continue", () => {
    const f = fns();
    render(<Welcome note {...f} />);
    expect(screen.getByText(/No problem/).textContent).toContain(
      "Settings > General > Take the tour",
    );
    expect(document.activeElement).toBe(screen.getByText("Continue"));
    fireEvent.click(screen.getByText("Continue"));
    expect(f.onContinue).toHaveBeenCalled();
  });
});
