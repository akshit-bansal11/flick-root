import { describe, expect, it } from "vitest";
import {
  acceleratorCaps,
  acceleratorChord,
  ghostInput,
  goalMet,
  isAccelerator,
  isLastStep,
  nextStep,
  prevStep,
  TOUR_STEPS,
  type TourEvent,
  type TourView,
} from "./steps";

const view = (v: Partial<Omit<TourView, "seen">> & { seen?: TourEvent[] } = {}): TourView => ({
  mode: "none",
  input: "",
  attachment: false,
  ...v,
  seen: new Set(v.seen ?? []),
});
const at = (id: string) => TOUR_STEPS.findIndex((s) => s.id === id);

describe("tour steps", () => {
  it("run open to done with unique ids", () => {
    expect(TOUR_STEPS[0]?.id).toBe("open");
    expect(TOUR_STEPS.at(-1)?.id).toBe("done");
    expect(TOUR_STEPS).toHaveLength(11);
    expect(new Set(TOUR_STEPS.map((s) => s.id)).size).toBe(TOUR_STEPS.length);
  });
  it("follow the prefix order and end with the five prefixes", () => {
    expect(TOUR_STEPS.map((s) => s.id)).toEqual([
      "open",
      "plain",
      "find",
      "commands",
      "calc",
      "ask",
      "explain",
      "around",
      "themes",
      "done",
    ]);
    expect(TOUR_STEPS.at(-1)?.keys).toEqual(["/", ">", "=", "?"]);
  });
  it("next ends after the last step; back stops at the first", () => {
    expect(nextStep(0)).toBe(1);
    expect(nextStep(TOUR_STEPS.length - 1)).toBeNull();
    expect(isLastStep(TOUR_STEPS.length - 1)).toBe(true);
    expect(prevStep(0)).toBe(0);
    expect(prevStep(3)).toBe(2);
  });
  it("each goal needs its own events", () => {
    expect(goalMet(at("open"), view())).toBe(false);
    expect(goalMet(at("open"), view({ seen: ["hotkey"] }))).toBe(true);
    expect(goalMet(at("plain"), view({ seen: ["launchApp"] }))).toBe(true);
    expect(goalMet(at("plain"), view({ seen: ["openFile"] }))).toBe(false);
    expect(goalMet(at("find"), view({ seen: ["launchApp"] }))).toBe(true);
    expect(goalMet(at("find"), view({ seen: ["openFile"] }))).toBe(true);
    expect(goalMet(at("find"), view({ seen: ["copied"] }))).toBe(false);
    expect(goalMet(at("commands"), view({ seen: ["runCommand"] }))).toBe(true);
    expect(goalMet(at("commands"), view({ seen: ["launchApp"] }))).toBe(false);
    expect(goalMet(at("calc"), view({ mode: "calc", seen: ["copied"] }))).toBe(true);
    expect(goalMet(at("calc"), view({ mode: "commands", seen: ["copied"] }))).toBe(false);
    expect(goalMet(at("calc"), view({ mode: "calc" }))).toBe(false);
    expect(goalMet(at("ask"), view({ seen: ["modelSwitched"] }))).toBe(false);
    expect(goalMet(at("ask"), view({ seen: ["modelSwitched", "askSent"] }))).toBe(true);
    expect(goalMet(at("explain"), view({ seen: ["captured"] }))).toBe(false);
    expect(goalMet(at("explain"), view({ seen: ["captured"], attachment: true }))).toBe(true);
    expect(goalMet(at("around"), view({ mode: "find" }))).toBe(false);
    expect(goalMet(at("around"), view({ mode: "find", seen: ["left"] }))).toBe(true);
    expect(goalMet(at("themes"), view({ seen: ["theme"] }))).toBe(true);
    expect(goalMet(at("done"), view({ seen: ["theme", "left"] }))).toBe(false);
  });
  it("step text reads its shortcuts from the table", () => {
    const body = (id: string) => TOUR_STEPS[at(id)]?.body ?? "";
    expect(body("calc")).toContain("ctrl shift c");
    expect(body("around")).toContain("f1");
    expect(body("around")).toContain("backspace");
    expect(body("around")).toContain("ctrl tab");
    expect(body("around")).toContain("ctrl shift tab");
    expect(body("plain")).toContain("recent");
    expect(TOUR_STEPS[at("ask")]?.keys).toContain("Tab");
  });
  it("ghostInput sends the prefix first, then the text after it", () => {
    expect(ghostInput("/")).toBe("/");
    expect(ghostInput("/ ")).toBe("");
    expect(ghostInput("/ spo")).toBe("spo");
    expect(ghostInput("spo")).toBe("spo");
  });
  it("maps accelerators onto chords", () => {
    expect(acceleratorChord("Alt+Space")).toBe("Alt+Space");
    expect(acceleratorChord("Shift+Control+k")).toBe("Ctrl+Shift+K");
    expect(acceleratorChord("")).toBeNull();
    expect(acceleratorCaps("Alt+Space")).toEqual(["Alt", "Space"]);
    const e = { key: " ", ctrlKey: false, shiftKey: false, altKey: true, metaKey: false };
    expect(isAccelerator("Alt+Space", e)).toBe(true);
    expect(isAccelerator("Ctrl+Space", e)).toBe(false);
  });
});
