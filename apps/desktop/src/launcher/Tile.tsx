import type { CSSProperties } from "react";
import type { Item } from "../types";

/** Stable hue 0-359 from the title so a letter tile keeps its color. */
function hue(title: string): number {
  let h = 0;
  for (const c of title) h = (h * 31 + (c.codePointAt(0) ?? 0)) % 360;
  return h;
}

export function Tile({ item }: { item: Item }) {
  const icon = item.icon;
  if (icon?.type === "dataUrl") return <img className="tile" src={icon.value} alt="" />;
  const letter = (icon?.type === "glyph" ? icon.value : item.title).slice(0, 1).toUpperCase();
  return (
    <span className="tile tile-letter" style={{ "--h": hue(item.title) } as CSSProperties}>
      {letter}
    </span>
  );
}
