import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SHORTCUTS } from "../shortcuts";
import { ShortcutSheet } from "./ShortcutSheet";

describe("ShortcutSheet", () => {
  it("is a labelled dialog with a heading", () => {
    render(<ShortcutSheet />);
    const dialog = screen.getByRole("dialog", { name: "Keyboard shortcuts" });
    expect(dialog).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2 }).textContent).toBe("Keyboard shortcuts");
  });

  it("lists every table entry label with keys in <kbd>", () => {
    const { container } = render(<ShortcutSheet />);
    for (const s of SHORTCUTS)
      expect(screen.getAllByText(s.label, { exact: false }).length).toBeGreaterThan(0);
    expect(container.querySelectorAll("kbd").length).toBeGreaterThan(SHORTCUTS.length);
    expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toContain(
      "Find",
    );
  });
});
