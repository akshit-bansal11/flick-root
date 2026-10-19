import { Fragment } from "react";
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
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

export function Results({ items, selected, query, onActivate }: Props) {
  // Window of MAX_ROWS rows that always contains the selection.
  const start = Math.max(0, Math.min(selected - MAX_ROWS + 1, items.length - MAX_ROWS));
  return (
    <>
      <div id={LISTBOX_ID} role="listbox" aria-label="Results" className="results">
        {items.slice(start, start + MAX_ROWS).map((item, k) => {
          const index = start + k;
          const label = groupLabel(item.kind);
          const prev = items[index - 1];
          return (
            <Fragment key={item.id}>
              {(!prev || groupLabel(prev.kind) !== label) && (
                <div className="group" aria-hidden="true">
                  {label}
                </div>
              )}
              <ResultRow
                item={item}
                index={index}
                id={optionId(index)}
                selected={index === selected}
                query={query}
                onActivate={onActivate}
              />
            </Fragment>
          );
        })}
      </div>
      {items.length === 0 && query.trim() !== "" && <div className="empty">No matches</div>}
    </>
  );
}
