import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Tutorial } from "./Tutorial";
import { TUTORIAL_STEPS } from "./tutorialSteps";

const setup = (step: number) => {
  const h = { onNext: vi.fn(), onBack: vi.fn(), onSkip: vi.fn(), onExample: vi.fn() };
  render(<Tutorial step={step} {...h} />);
  return h;
};

describe("Tutorial", () => {
  afterEach(cleanup);

  it("renders a labelled non-modal dialog with the step counter", () => {
    setup(1);
    const d = screen.getByRole("dialog");
    expect(d.getAttribute("aria-modal")).toBe("false");
    expect(screen.getByRole("heading").id).toBe(d.getAttribute("aria-labelledby"));
    expect(screen.getByText(`2 / ${TUTORIAL_STEPS.length}`)).toBeTruthy();
  });
  it("buttons call back; the example chip types its example", () => {
    const h = setup(1);
    fireEvent.click(screen.getByText("Next"));
    fireEvent.click(screen.getByText("Back"));
    fireEvent.click(screen.getByText("Skip"));
    fireEvent.click(screen.getByText(/^Try:/));
    expect(h.onNext).toHaveBeenCalled();
    expect(h.onBack).toHaveBeenCalled();
    expect(h.onSkip).toHaveBeenCalled();
    expect(h.onExample).toHaveBeenCalledWith("! notepad");
  });
  it("first step disables Back; last step says Finish", () => {
    setup(0);
    expect((screen.getByText("Back") as HTMLButtonElement).disabled).toBe(true);
    cleanup();
    setup(TUTORIAL_STEPS.length - 1);
    expect(screen.getByText("Finish")).toBeTruthy();
  });
});
