// Pure model of the Settings > Appearance theme picker: no React, no IPC, no DOM.
import type { ThemeView } from "../theme";

export type PickerChip = "all" | "dark" | "light" | "flick" | "ide" | "imported";

/** Chips in display order. */
export const PICKER_CHIPS: readonly PickerChip[] = [
  "all",
  "dark",
  "light",
  "flick",
  "ide",
  "imported",
];

/** Appearance field a picked theme is saved to: its own variant's slot. */
export type ThemeSlot = "darkTheme" | "lightTheme";

/** The slot a theme is saved to: its own variant's. */
export const slotOf = (t: ThemeView): ThemeSlot => (t.dark ? "darkTheme" : "lightTheme");

export type PickerKey = "ArrowUp" | "ArrowDown" | "Home" | "End" | "Enter" | "Escape";

export interface PickerState {
  themes: ThemeView[];
  query: string;
  chip: PickerChip;
  /** Id of the highlighted theme; null only when the filtered list is empty. */
  selected: string | null;
}

export type PickerEvent =
  | { type: "query"; query: string }
  | { type: "chip"; chip: PickerChip }
  | { type: "key"; key: PickerKey }
  | { type: "pick"; id: string }
  | { type: "loaded"; themes: ThemeView[] };

export type PickerEffect =
  | { type: "preview"; id: string }
  | { type: "apply"; id: string; slot: ThemeSlot }
  | { type: "revert" }
  | { type: "close" };

const chipMatch: Record<PickerChip, (t: ThemeView) => boolean> = {
  all: () => true,
  dark: (t) => t.dark,
  light: (t) => !t.dark,
  flick: (t) => t.source === "builtin",
  ide: (t) => t.source === "port",
  imported: (t) => t.source === "imported",
};

/** Themes matching the chip and a case-insensitive name substring, in input order. */
export function filterThemes(themes: ThemeView[], query: string, chip: PickerChip): ThemeView[] {
  const q = query.trim().toLowerCase();
  return themes.filter((t) => chipMatch[chip](t) && t.name.toLowerCase().includes(q));
}

/** How many themes each chip would show, ignoring the text filter. */
export function chipCounts(themes: ThemeView[]): Record<PickerChip, number> {
  return {
    all: themes.filter(chipMatch.all).length,
    dark: themes.filter(chipMatch.dark).length,
    light: themes.filter(chipMatch.light).length,
    flick: themes.filter(chipMatch.flick).length,
    ide: themes.filter(chipMatch.ide).length,
    imported: themes.filter(chipMatch.imported).length,
  };
}

/** The list the picker currently shows. */
export function visibleThemes(state: PickerState): ThemeView[] {
  return filterThemes(state.themes, state.query, state.chip);
}

/** Initial state; `selected` is `currentId` when it is visible, else the first row. */
export function initPicker(
  themes: ThemeView[],
  chip: PickerChip,
  currentId: string | null,
): PickerState {
  const list = filterThemes(themes, "", chip);
  const selected = list.find((t) => t.id === currentId)?.id ?? list[0]?.id ?? null;
  return { themes, query: "", chip, selected };
}

type Step = { state: PickerState; effects: PickerEffect[] };

/** Moves the selection; emits a preview for the new theme, or revert when nothing is selectable. */
function select(state: PickerState, selected: string | null): Step {
  if (selected === state.selected) return { state, effects: [] };
  const effects: PickerEffect[] =
    selected === null ? [{ type: "revert" }] : [{ type: "preview", id: selected }];
  return { state: { ...state, selected }, effects };
}

/** Keeps the selection if still shown after the list changed, else the first row. */
function refilter(state: PickerState): Step {
  const list = visibleThemes(state);
  const keep = list.some((t) => t.id === state.selected);
  return select(state, keep ? state.selected : (list[0]?.id ?? null));
}

function move(state: PickerState, key: PickerKey): Step {
  const list = visibleThemes(state);
  if (list.length === 0) return { state, effects: [] };
  const at = list.findIndex((t) => t.id === state.selected);
  let next = 0;
  if (key === "ArrowDown") next = at < 0 ? 0 : (at + 1) % list.length;
  else if (key === "ArrowUp") next = (Math.max(at, 0) - 1 + list.length) % list.length;
  else if (key === "End") next = list.length - 1;
  return select(state, list[next]?.id ?? null);
}

/** One picker transition; pure, so the UI runs the returned effects. */
export function reducePicker(state: PickerState, event: PickerEvent): Step {
  switch (event.type) {
    case "query":
      return refilter({ ...state, query: event.query });
    case "chip":
      return refilter({ ...state, chip: event.chip });
    case "pick":
      return visibleThemes(state).some((t) => t.id === event.id)
        ? select(state, event.id)
        : { state, effects: [] };
    case "loaded":
      return refilter({ ...state, themes: event.themes });
    case "key": {
      switch (event.key) {
        case "Enter": {
          const t = state.themes.find((x) => x.id === state.selected);
          if (!t) return { state, effects: [] };
          return {
            state,
            effects: [{ type: "apply", id: t.id, slot: slotOf(t) }],
          };
        }
        case "Escape":
          return { state, effects: [{ type: "revert" }, { type: "close" }] };
        case "ArrowUp":
        case "ArrowDown":
        case "Home":
        case "End":
          return move(state, event.key);
      }
    }
  }
}
