import { cleanup, createEvent, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LaunchMode } from "../types";
import { ModeTabs } from "./ModeTabs";

const modes: ("find" | "calc")[] = ["find", "calc"];
const noop = () => {};

afterEach(cleanup);

describe("ModeTabs", () => {
  it("renders all plus only the enabled modes", () => {
    render(<ModeTabs modes={modes} mode="none" onSelect={noop} />);
    const names = screen.getAllByRole("tab").map((t) => t.textContent);
    expect(names).toEqual(["all", "/open", "=calc"]);
  });

  it("the note shows the chord range of the enabled modes", () => {
    const note = () => document.querySelector(".tabs-note")?.textContent;
    const { rerender } = render(<ModeTabs modes={modes} mode="none" onSelect={noop} />);
    expect(note()).toBe("ctrl+1…3");
    rerender(<ModeTabs modes={["calc"]} mode="none" onSelect={noop} />);
    expect(note()).toBe("ctrl+3");
  });

  it("aria-selected follows the mode", () => {
    const { rerender } = render(<ModeTabs modes={modes} mode="none" onSelect={noop} />);
    const selected = () =>
      screen
        .getAllByRole("tab")
        .filter((t) => t.getAttribute("aria-selected") === "true")
        .map((t) => t.textContent);
    expect(selected()).toEqual(["all"]);
    rerender(<ModeTabs modes={modes} mode="calc" onSelect={noop} />);
    expect(selected()).toEqual(["=calc"]);
  });

  it("a click calls onSelect with the mode", () => {
    const onSelect = vi.fn<(mode: LaunchMode) => void>();
    render(<ModeTabs modes={modes} mode="none" onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("tab", { name: "/open" }));
    expect(onSelect).toHaveBeenLastCalledWith("find");
    fireEvent.click(screen.getByRole("tab", { name: "all" }));
    expect(onSelect).toHaveBeenLastCalledWith("none");
  });

  it("mousedown is default-prevented so the input keeps focus", () => {
    render(<ModeTabs modes={modes} mode="none" onSelect={noop} />);
    const tab = screen.getByRole("tab", { name: "/open" });
    const down = createEvent.mouseDown(tab);
    fireEvent(tab, down);
    expect(down.defaultPrevented).toBe(true);
  });
});
