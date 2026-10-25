import type { ThemeView } from "../theme";
import type { Item, ItemKind } from "../types";

const GROUP: Record<ItemKind, string> = {
  app: "Apps",
  command: "Commands",
  file: "Files",
  folder: "Folders",
  project: "Projects",
  process: "Processes",
  result: "Results",
  media: "Results",
  capture: "Results",
  choice: "Results",
};

const NAME: Record<ItemKind, string> = {
  app: "Application",
  command: "Command",
  file: "File",
  folder: "Folder",
  project: "Project",
  process: "Process",
  result: "Result",
  media: "Media",
  capture: "Capture",
  choice: "Choice",
};

/** True for apps found by the drive scan (payload.program), not Start-menu apps. */
export const isProgram = (item: Item): boolean =>
  typeof item.payload === "object" &&
  item.payload !== null &&
  "program" in item.payload &&
  item.payload.program === true;

/** True for rows `home_items` produced (payload.home === "recent"). */
const isHome = (item: Item): boolean =>
  typeof item.payload === "object" &&
  item.payload !== null &&
  "home" in item.payload &&
  item.payload.home === "recent";

/** Group label of the blank tab's recent picks; rendering prefixes it with `# `. */
export const HOME_GROUP = "recent";

/** Source of the rows the `> theme` quick pick shows. */
export const THEME_SOURCE = "ui:theme";

const isThemeView = (v: unknown): v is ThemeView =>
  typeof v === "object" &&
  v !== null &&
  "id" in v &&
  typeof v.id === "string" &&
  "roles" in v &&
  "ratios" in v;

/** The theme a quick-pick row carries, and whether it is the saved one; null for any other row. */
export function themeRow(item: Item | undefined): { theme: ThemeView; current: boolean } | null {
  const p = item?.payload;
  if (item?.source !== THEME_SOURCE || typeof p !== "object" || p === null) return null;
  if (!("theme" in p) || !isThemeView(p.theme)) return null;
  return { theme: p.theme, current: "current" in p && p.current === true };
}

/** Group label shown above a run of same-group results. */
export const groupLabel = (item: Item): string =>
  item.source === THEME_SOURCE
    ? "Themes"
    : item.source === "ui"
      ? "Search instead"
      : isHome(item)
        ? HOME_GROUP
        : isProgram(item)
          ? "Programs"
          : GROUP[item.kind];

/** Human name of a kind, used as the subtitle when an item has none. */
export const kindName = (kind: ItemKind): string => NAME[kind];

/** Short kind label at the end of a row. */
export function rowKind(item: Item): string {
  const pick = themeRow(item);
  if (pick) return pick.current ? "current" : pick.theme.dark ? "dark" : "light";
  return isProgram(item) ? "Program" : item.kind;
}
