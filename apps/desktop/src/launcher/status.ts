import { type Condition, inScope, keyWords } from "../shortcuts";
import type { Item, ItemKind, LaunchMode } from "../types";

export interface KindCount {
  kind: ItemKind;
  n: number;
}

export interface StatusHint {
  id: string;
  /** Chord as words ("enter", "ctrl+enter") or the arrows "↑↓". */
  keys: string;
  label: string;
}

export interface Status {
  /** "2 apps · 2 files · 1 folder"; empty when there are no rows. */
  text: string;
  /** At most three. */
  hints: StatusHint[];
}

const NOUN: Record<ItemKind, readonly [string, string]> = {
  app: ["app", "apps"],
  command: ["command", "commands"],
  file: ["file", "files"],
  folder: ["folder", "folders"],
  project: ["project", "projects"],
  process: ["process", "processes"],
  result: ["result", "results"],
  media: ["media item", "media items"],
  capture: ["capture", "captures"],
  choice: ["choice", "choices"],
};

const MAX_HINTS = 3;

/** Per-kind counts in order of first appearance, so the text follows the ranking. */
export function countsOf(items: readonly Item[]): KindCount[] {
  const out: KindCount[] = [];
  for (const { kind, source } of items) {
    if (source === "ui") continue;
    const hit = out.find((c) => c.kind === kind);
    if (hit) hit.n += 1;
    else out.push({ kind, n: 1 });
  }
  return out;
}

/** "2 apps · 2 files · 1 folder". */
export const countText = (counts: readonly KindCount[]): string =>
  counts.map(({ kind, n }) => `${n} ${NOUN[kind][n === 1 ? 0 : 1]}`).join(" · ");

/** Status line: the count text plus up to three key hints from the shortcut table. */
export function statusHints(
  mode: LaunchMode,
  ctx: Partial<Record<Condition, boolean>>,
  counts: readonly KindCount[],
): Status {
  const scoped = inScope(mode)
    .filter((s) => s.foot && (!s.when || ctx[s.when]))
    .map((s) => ({
      id: s.id,
      keys: keyWords(s.keys),
      label: s.id === "open" && ctx.copyValue ? "copy" : (s.foot ?? ""),
    }));
  // Ask mode has no list to move through.
  const move = { id: "move", keys: "↑↓", label: "move" };
  const hints = mode === "query" ? scoped : [move, ...scoped];
  return { text: countText(counts), hints: hints.slice(0, MAX_HINTS) };
}
