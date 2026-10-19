import type { ItemKind } from "../types";

const GROUP: Record<ItemKind, string> = {
  app: "Apps",
  command: "Commands",
  file: "Files",
  folder: "Files",
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

/** Group label shown above a run of same-group results. */
export const groupLabel = (kind: ItemKind): string => GROUP[kind];

/** Human name of a kind, used as the subtitle when an item has none. */
export const kindName = (kind: ItemKind): string => NAME[kind];
