import type { Item } from "../types";
import { highlight } from "./highlight";
import { kindName, rowKind } from "./kinds";
import { Tile } from "./Tile";

interface Props {
  item: Item;
  id: string;
  index: number;
  selected: boolean;
  query: string;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

const ACTION_LABEL = {
  primary: "Open",
  admin: "Admin",
  location: "Show",
  copyPath: "Copy",
  copyResult: "Copy",
  kill: "Kill",
  custom: "Run",
} as const;

/** Hint shown on the selected row: its primary (first) action. */
export function primaryLabel(item: Item): string {
  const first = item.actions[0];
  return first ? ACTION_LABEL[first.type] : "Open";
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
      <Tile item={item} />
      <span className="row-text">
        <span className="row-title">
          {highlight(item.title, query).map((s) =>
            s.hit ? <b key={s.at}>{s.text}</b> : <span key={s.at}>{s.text}</span>,
          )}
        </span>
        <span className="row-sub">{item.subtitle || kindName(item.kind)}</span>
      </span>
      <span className="row-meta" aria-hidden="true">
        {selected ? `↵ ${primaryLabel(item)}` : rowKind(item)}
      </span>
    </div>
  );
}
