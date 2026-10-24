import { Fragment, type ReactNode } from "react";
import type { Item } from "../types";
import { groupLabel } from "./kinds";
import { ResultRow } from "./ResultRow";

export const LISTBOX_ID = "launcher-list";
export const optionId = (i: number) => `launcher-opt-${i}`;
const MAX_ROWS = 8;

interface Props {
  items: Item[];
  selected: number;
  query: string;
  /** True while a search is in flight; the empty state waits for its results. */
  pending?: boolean;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
  /** Right pane content for the selected row. */
  detail?: ReactNode;
}

export function Results({ items, selected, query, pending = false, onActivate, detail }: Props) {
  // Window of MAX_ROWS rows that always contains the selection.
  const start = Math.max(0, Math.min(selected - MAX_ROWS + 1, items.length - MAX_ROWS));
  const visible = items.slice(start, start + MAX_ROWS);
  return (
    <div className="split">
      <div id={LISTBOX_ID} role="listbox" aria-label="Results" className="left">
        {visible.map((item, k) => {
          const index = start + k;
          const label = groupLabel(item);
          const prev = items[index - 1];
          return (
            <Fragment key={item.id}>
              {(!prev || groupLabel(prev) !== label) && (
                <div className="group" aria-hidden="true">
                  {`# ${label.toLowerCase()}`}
                </div>
              )}
              <ResultRow
                item={item}
                id={optionId(index)}
                index={index}
                selected={index === selected}
                query={query}
                onActivate={onActivate}
              />
            </Fragment>
          );
        })}
      </div>
      <div className="right">
        {detail}
        {items.length === 0 && !pending && query.trim() !== "" && (
          <div className="empty">No matches</div>
        )}
      </div>
    </div>
  );
}
