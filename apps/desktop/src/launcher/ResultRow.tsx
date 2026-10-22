import { Keycaps } from "../Keycaps";
import type { Item } from "../types";
import { highlight } from "./highlight";
import { kindName, rowKind } from "./kinds";
import { Tile } from "./Tile";

interface Props {
  item: Item;
  id: string;
  index: number;
  selected: boolean;
  /** Position among the visible rows; the first few stagger in. */
  order: number;
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

export function ResultRow({ item, id, index, selected, order, query, onActivate }: Props) {
  return (
    <div
      id={id}
      role="option"
      tabIndex={-1}
      aria-selected={selected}
      data-i={order < 6 ? order : undefined}
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
      {selected ? (
        <span className="row-act" aria-hidden="true">
          <Keycaps caps={["↵"]} />
          {primaryLabel(item)}
        </span>
      ) : (
        <span className="row-meta" aria-hidden="true">
          {rowKind(item)}
        </span>
      )}
    </div>
  );
}
