import { screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OnOff } from "./OnOff";

let root: Root | undefined;

function mount(checked: boolean, onChange: (next: boolean) => void) {
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<OnOff checked={checked} onChange={onChange} label="Launch" />));
}

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
});

function pressed(name: string) {
  return screen.getByRole("button", { name }).getAttribute("aria-pressed");
}

describe("OnOff", () => {
  it("is a labelled group with the current side pressed", () => {
    mount(true, () => {});
    expect(screen.getByRole("group", { name: "Launch" })).toBeTruthy();
    expect([pressed("on"), pressed("off")]).toEqual(["true", "false"]);
  });

  it("shows off pressed when unchecked", () => {
    mount(false, () => {});
    expect([pressed("on"), pressed("off")]).toEqual(["false", "true"]);
  });

  it("calls onChange with the other side, and not for the pressed one", () => {
    const onChange = vi.fn();
    mount(false, onChange);
    act(() => screen.getByRole("button", { name: "off" }).click());
    expect(onChange).not.toHaveBeenCalled();
    act(() => screen.getByRole("button", { name: "on" }).click());
    expect(onChange).toHaveBeenCalledWith(true);
    act(() => root?.unmount());
    mount(true, onChange);
    act(() => screen.getByRole("button", { name: "off" }).click());
    expect(onChange).toHaveBeenLastCalledWith(false);
  });
});
