import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ActionKind, Item, ItemKind } from "../types";
import { DetailPane } from "./DetailPane";

const make = (
  kind: ItemKind,
  id: string,
  actions: ActionKind[],
  payload: unknown = null,
): Item => ({
  id,
  kind,
  title: "Alpha",
  subtitle: "",
  icon: null,
  score: 1,
  source: "test",
  actions,
  payload,
});

const app = make("app", "app:a", [{ type: "primary" }, { type: "admin" }, { type: "location" }], {
  parsingName: "C:/Apps/alpha.exe",
});
const file = make("file", "file:D:/docs/a.txt", [{ type: "primary" }, { type: "copyPath" }]);

describe("DetailPane", () => {
  it("shows title, tag and the path fact for an app", () => {
    const { container } = render(<DetailPane item={app} ctx={{}} onPress={() => {}} />);
    expect(screen.getByRole("heading", { name: "Alpha" }).className).toBe("detail-title");
    expect(container.querySelector(".tag")?.textContent).toBe("app");
    const fact = container.querySelector("dl.facts .fact");
    expect(fact?.querySelector("dt")?.textContent).toBe("path");
    expect(fact?.querySelector("dd")?.textContent).toBe("C:/Apps/alpha.exe");
  });

  it("shows the path fact for a file", () => {
    const { container } = render(<DetailPane item={file} ctx={{}} onPress={() => {}} />);
    expect(container.querySelector(".tag")?.textContent).toBe("file");
    expect(container.querySelector("dl.facts dd")?.textContent).toBe("D:/docs/a.txt");
  });

  it("lists actions in order with key words, primary first", () => {
    const { container } = render(<DetailPane item={app} ctx={{}} onPress={() => {}} />);
    const buttons = [...container.querySelectorAll("button.detail-act")];
    expect(buttons.map((b) => b.firstChild?.textContent)).toEqual([
      "open",
      "run as administrator",
      "open file location",
      "pin",
    ]);
    expect(buttons[1]?.querySelector(".detail-keys")?.textContent).toBe("ctrl+shift+enter");
    expect(buttons[0]?.className).toBe("detail-act pri");
    expect(buttons[1]?.className).toBe("detail-act");
  });

  it("clicking an action sends its chord; run sends the open chord", () => {
    const onPress = vi.fn();
    const cmd = make("command", "cmd:x", [{ type: "primary" }]);
    const { rerender } = render(<DetailPane item={app} ctx={{}} onPress={onPress} />);
    fireEvent.click(screen.getByRole("button", { name: /run as administrator/i }));
    expect(onPress).toHaveBeenLastCalledWith("Ctrl+Shift+Enter");
    rerender(<DetailPane item={cmd} ctx={{}} onPress={onPress} />);
    fireEvent.click(screen.getByRole("button", { name: /^run/ }));
    expect(onPress).toHaveBeenLastCalledWith("Enter");
  });

  it("an app shows pin or unpin by ctx, with the pinned tag, and pin sends Ctrl+P", () => {
    const onPress = vi.fn();
    const { container, rerender } = render(<DetailPane item={app} ctx={{}} onPress={onPress} />);
    fireEvent.click(screen.getByRole("button", { name: /^pin/ }));
    expect(onPress).toHaveBeenLastCalledWith("Ctrl+P");
    rerender(<DetailPane item={app} ctx={{ pinned: true }} onPress={onPress} />);
    expect(container.querySelector(".tag")?.textContent).toBe("app · pinned");
    expect(screen.getByRole("button", { name: /^unpin/ })).toBeTruthy();
  });

  it("mousedown on an action is default-prevented so the input keeps focus", () => {
    render(<DetailPane item={app} ctx={{}} onPress={() => {}} />);
    const btn = screen.getByRole("button", { name: /run as administrator/i });
    const down = createEvent.mouseDown(btn);
    fireEvent(btn, down);
    expect(down.defaultPrevented).toBe(true);
  });

  it("renders no facts section when there are none", () => {
    const bare = make("app", "app:b", [{ type: "primary" }]);
    const { container } = render(<DetailPane item={bare} ctx={{}} onPress={() => {}} />);
    expect(container.querySelector("dl")).toBeNull();
  });
});
