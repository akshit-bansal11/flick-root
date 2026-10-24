import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusLine } from "./StatusLine";
import type { Status } from "./status";

const status: Status = {
  text: "2 apps · 1 file",
  hints: [
    { id: "move", keys: "↑↓", label: "move" },
    { id: "open", keys: "enter", label: "open" },
    { id: "admin", keys: "ctrl+shift+enter", label: "admin" },
  ],
};

describe("StatusLine", () => {
  it("shows the counts and the hints as words", () => {
    const { container } = render(<StatusLine status={status} />);
    expect(container.querySelector(".count")?.textContent).toBe("2 apps · 1 file");
    expect([...container.querySelectorAll(".act")].map((a) => a.textContent)).toEqual([
      "↑↓ move",
      "enter open",
      "ctrl+shift+enter admin",
    ]);
    expect(container.querySelector(".act kbd")).toBeNull();
  });

  it("shows the copy toast with role status", () => {
    render(<StatusLine status={status} copied />);
    expect(screen.getByRole("status").textContent).toBe("Copied");
  });

  it("shows Copy failed, never Copied, when the copy failed", () => {
    render(<StatusLine status={status} copied copyFailed />);
    expect(screen.getByRole("status").textContent).toBe("Copy failed");
  });

  it("has no toast by default", () => {
    render(<StatusLine status={status} />);
    expect(screen.queryByRole("status")).toBeNull();
  });
});
