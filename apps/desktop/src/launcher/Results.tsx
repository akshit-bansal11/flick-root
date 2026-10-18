import type { Item } from "../types";
import { ResultRow } from "./ResultRow";

export const LISTBOX_ID = "launcher-list";
export const optionId = (i: number) => `launcher-opt-${i}`;
const MAX_ROWS = 5;

interface Props {
  items: Item[];
  selected: number;
  onActivate: (itemId: string) => void;
}

export function Results({ items, selected, onActivate }: Props) {
  // Window of MAX_ROWS rows that always contains the selection.
  const start = Math.max(0, Math.min(selected - MAX_ROWS + 1, items.length - MAX_ROWS));
  return (
    <div id={LISTBOX_ID} role="listbox" aria-label="Results" className="results">
      {items.slice(start, start + MAX_ROWS).map((item, k) => (
        <ResultRow
          key={item.id}
          item={item}
          id={optionId(start + k)}
          selected={start + k === selected}
          onActivate={onActivate}
        />
      ))}
    </div>
  );
}
