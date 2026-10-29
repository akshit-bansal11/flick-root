import { caps, capsOf, capsText, chordOf, fromEvent } from "../shortcuts";
import type { LaunchMode } from "../types";

/** Things the user did in the demo that a step can wait for. */
export type TourEvent =
  | "hotkey"
  | "launchApp"
  | "openFile"
  | "runCommand"
  | "openProject"
  | "copied"
  | "askSent"
  | "modelSwitched"
  | "captured"
  | "left"
  | "theme";

/** What a step's goal can look at: the demo as the user sees it. */
export interface TourView {
  mode: LaunchMode;
  input: string;
  attachment: boolean;
  /** Events seen since this step began. */
  seen: ReadonlySet<TourEvent>;
}

/** What the "Show me" button does. */
export type ShowKind = "type" | "hotkey" | "select" | "theme";

export interface TourStep {
  id: string;
  title: string;
  body: string;
  /** Key caps shown on the card. */
  keys: string[];
  /** Real hotkey caps to show instead of `keys`, read from settings. */
  hotkey?: "launcher" | "explain";
  /** Text "Show me" types, prefix included. */
  example?: string;
  show?: ShowKind;
  /** True once the user did what the step asks; the tour then moves on by itself. */
  goal?: (view: TourView) => boolean;
}

const key = capsText;
const saw = (v: TourView, ...e: TourEvent[]) => e.every((x) => v.seen.has(x));

export const TOUR_STEPS: readonly TourStep[] = [
  {
    id: "open",
    title: "Open the bar",
    body: "Press your hotkey from anywhere in Windows and the bar appears. Try it now.",
    keys: [],
    hotkey: "launcher",
    show: "hotkey",
    goal: (v) => saw(v, "hotkey"),
  },
  {
    id: "plain",
    title: "Type anything",
    body: "Type without a prefix to search apps and commands. An empty bar lists your recent picks. The last rows search files, the web or ask.",
    keys: [],
    example: "spo",
    show: "type",
    goal: (v) => saw(v, "launchApp"),
  },
  {
    id: "find",
    title: "/ Open apps and files",
    body: `Find apps, programs, files and folders. ${key("open")} opens one, ${key("location")} shows it in Explorer.`,
    keys: ["/", ...caps("open"), ...caps("location")],
    example: "/ spotify",
    show: "type",
    goal: (v) => saw(v, "launchApp") || saw(v, "openFile"),
  },
  {
    id: "commands",
    title: "> Commands",
    body: `Lock the PC, end a process, free a port. Type lock, then press ${key("open")} to run it.`,
    keys: [">", ...caps("open")],
    example: "> lock",
    show: "type",
    goal: (v) => saw(v, "runCommand"),
  },
  {
    id: "calc",
    title: "= Calc",
    body: `Math, units, currency and time zones. Type 5 km to mi, then press ${key("open")} to copy the answer.`,
    keys: ["=", ...caps("open")],
    example: "= 5 km to mi",
    show: "type",
    goal: (v) => v.mode === "calc" && saw(v, "copied"),
  },
  {
    id: "projects",
    title: "@ Projects",
    body: `Jump into a code project. Type a name, then press ${key("open")} to open it.`,
    keys: ["@", ...caps("open")],
    example: "@ flick",
    show: "type",
    goal: (v) => saw(v, "openProject"),
  },
  {
    id: "ask",
    title: "? Ask",
    body: `Ask your own AI. ${key("model")} switches the model, ${key("ask")} asks. Nothing is sent from this tour.`,
    keys: ["?", ...caps("model"), ...caps("ask")],
    example: "? what is a CIDR block",
    show: "type",
    goal: (v) => saw(v, "modelSwitched", "askSent"),
  },
  {
    id: "explain",
    title: "Explain this",
    body: "Select the sample text below, then press the Explain hotkey. You see the captured text before anything is sent.",
    keys: [],
    hotkey: "explain",
    show: "select",
    goal: (v) => v.attachment && saw(v, "captured"),
  },
  {
    id: "around",
    title: "Getting around",
    body: `${key("leave")} on an empty bar leaves a mode. ${key("tabNext")} and ${key("tabPrev")} switch mode tabs. Then ${key("go:find")} jumps to Open. ${key("sheet")} lists every shortcut and ${key("back")} steps back: close, clear, hide.`,
    keys: [...caps("leave"), ...caps("go:find"), ...caps("sheet")],
    example: ">",
    show: "type",
    goal: (v) => saw(v, "left") && v.mode === "find",
  },
  {
    id: "themes",
    title: "Themes",
    body: "Pick a theme and the bar restyles on the spot. In the bar, > theme opens the same quick pick. Settings > Appearance has more.",
    keys: [],
    show: "theme",
    goal: (v) => saw(v, "theme"),
  },
  {
    id: "done",
    title: "You are set",
    body: "Press the hotkey any time and pick a prefix. Find this tour again in Settings > General.",
    keys: ["/", ">", "=", "?", "@"],
  },
];

const LAST = TOUR_STEPS.length - 1;

/** Step after `step`, or null when it was the last (finish). */
export const nextStep = (step: number): number | null => (step >= LAST ? null : step + 1);

/** Step before `step`, never below the first. */
export const prevStep = (step: number): number => Math.max(0, step - 1);

export const isLastStep = (step: number): boolean => step >= LAST;

/** True when the step's goal is met (steps without a goal never complete by themselves). */
export const goalMet = (step: number, view: TourView): boolean =>
  TOUR_STEPS[step]?.goal?.(view) ?? false;

/** Wait after a goal is met, so the check animation plays before the next step. */
export const ADVANCE_MS = 600;

/** Ghost typing speed: one character every this many ms. */
export const GHOST_MS = 45;

/**
 * What the demo's `text` event carries when the ghost has typed `typed` of an example:
 * the prefix alone first (it picks the mode), then only the text after the prefix.
 * An example with no prefix is typed as it is.
 */
export const ghostInput = (typed: string): string =>
  /^[/>=?@]/.test(typed) && typed.length > 1 ? typed.slice(1).trimStart() : typed;

const ACCEL_NAMES: Record<string, string> = {
  control: "Ctrl",
  ctrl: "Ctrl",
  commandorcontrol: "Ctrl",
  cmdorctrl: "Ctrl",
  alt: "Alt",
  option: "Alt",
  shift: "Shift",
  super: "Win",
  win: "Win",
  meta: "Win",
  cmd: "Win",
};
const MOD_ORDER = ["Ctrl", "Alt", "Shift", "Win"];

/** Chord (shortcuts.ts form) for a Tauri accelerator such as "Shift+Alt+Space"; null when empty. */
export function acceleratorChord(accel: string): string | null {
  const parts = accel
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  const k = parts.pop();
  if (!k) return null;
  const mods = parts.map((p) => ACCEL_NAMES[p.toLowerCase()] ?? p);
  const ordered = MOD_ORDER.filter((m) => mods.includes(m));
  return [...ordered, k.length === 1 ? k.toUpperCase() : k].join("+");
}

/** Key caps for an accelerator: ["Alt", "Space"]. */
export const acceleratorCaps = (accel: string): string[] => {
  const c = acceleratorChord(accel);
  return c ? capsOf(c) : [];
};

/** True when a DOM key press is the accelerator. */
export function isAccelerator(accel: string, e: Parameters<typeof fromEvent>[0]): boolean {
  const want = acceleratorChord(accel);
  return want !== null && chordOf(fromEvent(e)) === want;
}

/** Registry ids of the theme choices offered by the Themes step, in display order. */
export const TOUR_THEMES: readonly string[] = [
  "flick-default",
  "flick-default-light",
  "frost",
  "midnight",
  "aurora",
  "ember",
];
