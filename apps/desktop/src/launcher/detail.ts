import { type Condition, shortcut } from "../shortcuts";
import type { ActionKind, Item } from "../types";
import { isProgram } from "./kinds";

export type DetailCtx = Partial<Record<Condition, boolean>>;

export interface DetailFact {
  label: string;
  value: string;
}

export interface DetailAction {
  /** Shortcut id this action fires, or "run" for a primary action that runs (commands). */
  id: string;
  label: string;
  /** Chord written as words, "ctrl+shift+enter". */
  keys: string;
  primary: boolean;
}

export interface Detail {
  title: string;
  tag: string;
  facts: DetailFact[];
  actions: DetailAction[];
}

/** A chord as words, lower case: "Ctrl+Shift+Enter" becomes "ctrl+shift+enter". */
export const keyWords = (chord: string): string => chord.toLowerCase();

const shortcutKeys = (id: string): string => keyWords(shortcut(id).keys);

const parsingName = (item: Item): string | undefined =>
  typeof item.payload === "object" &&
  item.payload !== null &&
  "parsingName" in item.payload &&
  typeof item.payload.parsingName === "string" &&
  item.payload.parsingName !== ""
    ? item.payload.parsingName
    : undefined;

/** Only data the item carries: no times, no versions. */
function factsOf(item: Item): DetailFact[] {
  switch (item.kind) {
    case "app": {
      // Drive-scan programs have no parsing name; their subtitle is the folder they live in.
      const path = parsingName(item) ?? (isProgram(item) && item.subtitle ? item.subtitle : null);
      return path ? [{ label: "path", value: path }] : [];
    }
    case "file":
    case "folder":
      return item.id.startsWith("file:")
        ? [{ label: "path", value: item.id.slice("file:".length) }]
        : [];
    case "project":
      return item.subtitle ? [{ label: "path", value: item.subtitle }] : [];
    case "command":
    case "process":
    case "result":
    case "media":
    case "capture":
    case "choice":
      return item.subtitle ? [{ label: "about", value: item.subtitle }] : [];
  }
}

const row = (id: string, label: string): DetailAction => ({
  id,
  label,
  keys: shortcutKeys(id),
  primary: false,
});

/** One action of the item as a pane row; null when it has no chord or is hidden. */
function actionOf(a: ActionKind, first: boolean, item: Item, ctx: DetailCtx): DetailAction | null {
  if (first && (a.type === "primary" || a.type === "custom")) {
    const run = item.kind === "command" || a.type === "custom";
    return {
      id: run ? "run" : "open",
      label: run ? "run" : "open",
      keys: shortcutKeys("open"),
      primary: true,
    };
  }
  switch (a.type) {
    case "primary":
      return null;
    case "location":
      return row("location", "open file location");
    case "admin":
      return row("admin", "run as administrator");
    case "copyPath":
      return row("copyPath", "copy path");
    case "copyResult":
      return row("copyResult", "copy result");
    case "kill":
      return ctx.armedProcess ? row("kill", "end process") : null;
    case "custom":
      // ponytail: a secondary custom action has no chord in the table, so it is not listed.
      return null;
  }
}

/** Right-pane data for the selected row; actions keep the item's own order. */
export function detailOf(item: Item, ctx: DetailCtx = {}): Detail {
  return {
    title: item.title,
    tag: isProgram(item) ? "program" : item.kind,
    facts: factsOf(item),
    actions: item.actions.flatMap((a, i) => {
      const row = actionOf(a, i === 0, item, ctx);
      return row ? [row] : [];
    }),
  };
}
