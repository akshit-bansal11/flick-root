import { APP_DISPLAY_NAME } from "../brand";
import { caps } from "../shortcuts";
import { acceleratorCaps } from "../tour/steps";

export type MenuAction = "open" | "settings" | "update" | "quit";

interface MenuItem {
  action: MenuAction;
  label: string;
  caps: string[];
}

export interface MenuState {
  items: MenuItem[];
  selected: number;
}

type MenuEvent = { type: "key"; key: string } | { type: "hover"; index: number };

type MenuEffect = { type: "run"; action: MenuAction } | { type: "close" };

/** The four menu rows in display order; Quit is last (the component draws a separator before it). */
export function buildItems(args: {
  hotkey: string;
  update: { version: string } | null;
}): MenuItem[] {
  return [
    { action: "open", label: `Open ${APP_DISPLAY_NAME}`, caps: acceleratorCaps(args.hotkey) },
    { action: "settings", label: "Settings", caps: caps("openSettings") },
    {
      action: "update",
      label: args.update ? `Install ${args.update.version}` : "Check for updates",
      caps: [],
    },
    { action: "quit", label: "Quit", caps: [] },
  ];
}

const wrap = (i: number, n: number) => (i + n) % n;

/** Pure transition: keyboard and hover move the selection, Enter/Space run it, Escape closes. */
export function reduceMenu(
  state: MenuState,
  event: MenuEvent,
): { state: MenuState; effects: MenuEffect[] } {
  const n = state.items.length;
  const to = (selected: number) => ({ state: { ...state, selected }, effects: [] });
  if (event.type === "hover") {
    return event.index >= 0 && event.index < n ? to(event.index) : { state, effects: [] };
  }
  switch (event.key) {
    case "ArrowDown":
      return to(wrap(state.selected + 1, n));
    case "ArrowUp":
      return to(wrap(state.selected - 1, n));
    case "Home":
      return to(0);
    case "End":
      return to(n - 1);
    case "Enter":
    case " ": {
      const item = state.items[state.selected];
      return { state, effects: item ? [{ type: "run", action: item.action }] : [] };
    }
    case "Escape":
      return { state, effects: [{ type: "close" }] };
    default:
      return { state, effects: [] };
  }
}
