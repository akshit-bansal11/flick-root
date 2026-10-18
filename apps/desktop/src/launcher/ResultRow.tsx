import type { Item } from "../types";
import { Tile } from "./Tile";

interface Props {
  item: Item;
  id: string;
  selected: boolean;
  onActivate: (itemId: string) => void;
}

export function ResultRow({ item, id, selected, onActivate }: Props) {
  return (
    <div
      id={id}
      role="option"
      tabIndex={-1}
      aria-selected={selected}
      className={selected ? "row sel" : "row"}
      onMouseDown={(e) => {
        e.preventDefault();
        onActivate(item.id);
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
