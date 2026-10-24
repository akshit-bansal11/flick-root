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
    for (const s of SHORTCUTS.filter((x) => x.scope !== "tour"))
      expect(screen.getAllByText(s.label, { exact: false }).length).toBeGreaterThan(0);
    expect(container.querySelectorAll("kbd").length).toBeGreaterThan(SHORTCUTS.length - 2);
    expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toContain(
      "Open",
    );
  });

  it("shows a chord as words in a single kbd", () => {
    const { container } = render(<ShortcutSheet />);
    const texts = [...container.querySelectorAll("kbd")].map((k) => k.textContent);
    expect(texts).toContain("ctrl+shift+enter");
    expect(texts.every((t) => t === t?.toLowerCase())).toBe(true);
    expect(container.querySelectorAll("kbd kbd").length).toBe(0);
  });

  it("never lists the tour scope", () => {
    render(<ShortcutSheet />);
    expect(screen.queryByText("Tour")).toBeNull();
    expect(screen.queryByText("Next tour step")).toBeNull();
  });
});
