import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TermRadio } from "./TermRadio";

afterEach(cleanup);

const options = [
  { value: "a", label: "Alpha", hint: "first" },
  { value: "b", label: "Beta" },
];
const mount = (value: string, onChange = vi.fn()) => {
  render(
    <TermRadio legend="Pick one" name="pick" value={value} options={options} onChange={onChange} />,
  );
  return onChange;
};

describe("TermRadio", () => {
  it("renders a group named by the legend with native same-name radios", () => {
    mount("a");
    expect(screen.getByRole("group", { name: "Pick one" })).toBeTruthy();
    const radios = screen.getAllByRole<HTMLInputElement>("radio");
    expect(radios).toHaveLength(2);
    for (const r of radios) {
      expect(r.type).toBe("radio");
      expect(r.name).toBe("pick");
    }
  });

  it("checks only the radio matching value", () => {
    mount("b");
    expect(screen.getByRole<HTMLInputElement>("radio", { name: /^Alpha/ }).checked).toBe(false);
    expect(screen.getByRole<HTMLInputElement>("radio", { name: /^Beta/ }).checked).toBe(true);
  });

  it("includes the hint in the accessible name", () => {
    mount("a");
    expect(screen.getByRole("radio", { name: "Alpha first" })).toBeTruthy();
  });

  it("calls onChange with the clicked radio's value", () => {
    const onChange = mount("a");
    fireEvent.click(screen.getByRole("radio", { name: /^Beta/ }));
    expect(onChange).toHaveBeenCalledWith("b");
  });
});
