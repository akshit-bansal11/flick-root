import type { LaunchMode } from "../types";

/** What a step's `done` can look at: the launcher as the user sees it. */
export interface TutorialView {
  mode: LaunchMode;
  input: string;
  itemsCount: number;
  thinkingModes: number;
}

export interface TutorialStep {
  id: string;
  title: string;
  body: string;
  /** Key caps shown on the card. */
  keys?: string[];
  /** What the example button types, prefix included. */
  example?: string;
  /** True once the user has tried the step; the tutorial then moves on by itself. */
  done?: (view: TutorialView) => boolean;
}

export const TUTORIAL_STEPS: readonly TutorialStep[] = [
  {
    id: "welcome",
    title: "Welcome",
    body: "One bar, four modes. Each starts with a prefix character. This tour takes a minute and you can skip it any time.",
    keys: [">", "!", "/", "?"],
  },
  {
    id: "apps",
    title: "! Apps and programs",
    body: "Type ! then a name to open any app or program on your drives. Enter opens it, Ctrl+Enter shows its location, Ctrl+Shift+Enter runs it as admin.",
    keys: ["!", "Enter", "Ctrl Enter"],
    example: "! notepad",
    done: (v) => v.mode === "run" && v.input.trim() !== "",
  },
  {
    id: "commands",
    title: "> Commands",
    body: "Math (2+2), units (5 km to mi), currency (250 usd in inr), time zones (5pm ist in pst), web search (g cats), projects, system commands (lock), ports (port 3000, proc chrome: arrow to the row, then Backspace ends it), media (play, pause, play lofi on spotify) and capture (log 250 lunch).",
    keys: [">", "↑ ↓", "Backspace"],
    example: "> 5 km to mi",
    done: (v) => v.mode === "commands" && v.itemsCount > 0,
  },
  {
    id: "find",
    title: "/ Find",
    body: "Search files and folders. Enter opens, Ctrl+Enter shows the location, Ctrl+C copies the path. Plain language works too: pdf from yesterday.",
    keys: ["/", "Enter", "Ctrl Enter", "Ctrl C"],
    example: "/ pdf from yesterday",
    done: (v) => v.mode === "find" && v.input.trim() !== "" && v.itemsCount > 0,
  },
  {
    id: "ask",
    title: "? Ask",
    body: "Ask your own AI provider. Tab and Shift+Tab switch thinking mode (provider and model), Ctrl+T toggles fast and think. No provider yet? Connect one in Settings > Providers.",
    keys: ["?", "Tab", "Ctrl T"],
    example: "? what is a CIDR block",
  },
  {
    id: "explain",
    title: "Explain this",
    body: "Select text in any app and press the shortcut. You see the captured text first, and text that looks like a secret is blocked.",
    keys: ["Alt", "Shift", "Space"],
  },
  {
    id: "settings",
    title: "Themes and Settings",
    body: "Pick a theme in Settings > Appearance. Open Settings from the tray icon. Press F1 here to replay this tour.",
    keys: ["F1"],
  },
  {
    id: "finish",
    title: "You are set",
    body: "Press the hotkey any time to start. Replay the tour from Settings > General.",
  },
];

const LAST = TUTORIAL_STEPS.length - 1;

/** Step after `step`, or null when it was the last (finish). */
export const nextStep = (step: number): number | null => (step >= LAST ? null : step + 1);

/** Step before `step`, never below the first. */
export const prevStep = (step: number): number => Math.max(0, step - 1);

export const isLastStep = (step: number): boolean => step >= LAST;

/** Step to show after the view changed: moves on once the current step is done. */
export function autoAdvance(step: number, view: TutorialView): number {
  const done = TUTORIAL_STEPS[step]?.done;
  return done?.(view) && step < LAST ? step + 1 : step;
}

/** Splits an example into its mode prefix and the text typed after it. */
export function splitExample(example: string): { prefix: string; text: string } {
  return { prefix: example.slice(0, 1), text: example.slice(1).trimStart() };
}
