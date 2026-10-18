import type { Item } from "../types";
import { Tile } from "./Tile";

interface Props {
  item: Item;
  id: string;
  index: number;
  selected: boolean;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

export function ResultRow({ item, id, index, selected, onActivate }: Props) {
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
        <span className="row-title">{item.title}</span>
        <span className="row-sub">{item.subtitle}</span>
      </span>
      {selected && <span className="row-meta">↵</span>}
    </div>
  );
}
