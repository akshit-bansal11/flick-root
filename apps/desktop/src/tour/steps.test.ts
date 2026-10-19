import { describe, expect, it } from "vitest";
import {
  autoAdvance,
  isLastStep,
  nextStep,
  prevStep,
  splitExample,
  TUTORIAL_STEPS,
  type TutorialView,
} from "./steps";

const view = (v: Partial<TutorialView> = {}): TutorialView => ({
  mode: "none",
  input: "",
  itemsCount: 0,
  thinkingModes: 0,
  ...v,
});
const at = (id: string) => TUTORIAL_STEPS.findIndex((s) => s.id === id);

describe("tutorial steps", () => {
  it("run welcome to finish with unique ids", () => {
    expect(TUTORIAL_STEPS[0]?.id).toBe("welcome");
    expect(TUTORIAL_STEPS.at(-1)?.id).toBe("finish");
    expect(new Set(TUTORIAL_STEPS.map((s) => s.id)).size).toBe(TUTORIAL_STEPS.length);
  });
  it("next ends after the last step; back stops at the first", () => {
    expect(nextStep(0)).toBe(1);
    expect(nextStep(TUTORIAL_STEPS.length - 1)).toBeNull();
    expect(isLastStep(TUTORIAL_STEPS.length - 1)).toBe(true);
    expect(prevStep(0)).toBe(0);
    expect(prevStep(3)).toBe(2);
  });
  it("welcome advances as soon as any mode is entered", () => {
    expect(autoAdvance(0, view())).toBe(0);
    expect(autoAdvance(0, view({ mode: "query" }))).toBe(1);
  });
  it("auto-advances only when the current step is done", () => {
    const apps = at("apps");
    expect(autoAdvance(apps, view({ mode: "run" }))).toBe(apps);
    expect(autoAdvance(apps, view({ mode: "run", input: "chr" }))).toBe(apps + 1);
    const cmd = at("commands");
    expect(autoAdvance(cmd, view({ mode: "commands", input: "2+2", itemsCount: 1 }))).toBe(cmd + 1);
    expect(autoAdvance(cmd, view({ mode: "run", input: "x", itemsCount: 1 }))).toBe(cmd);
    expect(autoAdvance(at("ask"), view({ mode: "query", input: "hi" }))).toBe(at("ask"));
  });
  it("step text reads its shortcuts from the table", () => {
    const body = (id: string) => TUTORIAL_STEPS[at(id)]?.body ?? "";
    expect(body("find")).toContain("Ctrl Shift C copies the path");
    expect(body("commands")).toContain("Delete ends it");
    expect(body("commands")).not.toContain("Backspace");
    expect(body("settings")).toContain("F1");
  });
  it("splits an example into prefix and text", () => {
    expect(splitExample("! notepad")).toEqual({ prefix: "!", text: "notepad" });
  });
});
