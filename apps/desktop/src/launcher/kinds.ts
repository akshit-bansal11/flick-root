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

/** Group label shown above a run of same-group results. */
export const groupLabel = (item: Item): string =>
  item.source === "ui"
    ? "Search instead"
    : isHome(item)
      ? HOME_GROUP
      : isProgram(item)
        ? "Programs"
        : GROUP[item.kind];

/** Human name of a kind, used as the subtitle when an item has none. */
export const kindName = (kind: ItemKind): string => NAME[kind];

/** Short kind label at the end of a row. */
export const rowKind = (item: Item): string => (isProgram(item) ? "Program" : item.kind);
