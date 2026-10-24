import type { Item } from "../types";
import { highlight } from "./highlight";
import { rowKind } from "./kinds";

interface Props {
  item: Item;
  id: string;
  index: number;
  selected: boolean;
  query: string;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

export function ResultRow({ item, id, index, selected, query, onActivate }: Props) {
  return (
    <div
      id={id}
      role="option"
      tabIndex={-1}
      aria-selected={selected}
      className={selected ? "row sel" : "row"}
      onMouseDown={(e) => {
        e.preventDefault();
        onActivate(index, { ctrl: e.ctrlKey, shift: e.shiftKey });
      }}
    >
      <span className="row-mark" aria-hidden="true">
        {selected && "›"}
      </span>
      <span className="row-title">
        {highlight(item.title, query).map((s) =>
          s.hit ? <mark key={s.at}>{s.text}</mark> : <span key={s.at}>{s.text}</span>,
        )}
      </span>
      {item.source !== "ui" && (
        <span className="row-meta" aria-hidden="true">
          {rowKind(item)}
        </span>
      )}
    </div>
  );
}
