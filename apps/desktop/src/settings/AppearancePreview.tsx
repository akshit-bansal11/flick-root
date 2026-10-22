import { useRef } from "react";
import { Keycaps } from "../Keycaps";
import { Lens } from "../Lens";

/**
 * A static mini launcher over a decorative wallpaper. It reads the live theme tokens, so it follows
 * the draft theme and the glass slider while they change. Decorative: hidden from assistive tech.
 */
export function AppearancePreview() {
  const lens = useRef<HTMLDivElement>(null);
  return (
    <div className="s-pv" aria-hidden="true">
      <div className="s-pv-panel">
        <div className="s-pv-bar">
          <span className="s-pv-chip">
            <span className="s-pv-glyph">=</span>
            Calc
          </span>
          <span className="s-pv-query">2.5 kg to lb</span>
        </div>
        <div className="s-pv-list" data-lens="1">
          <Lens ref={lens} />
          <div className="s-pv-row">
            <span className="s-pv-name">5.51 lb</span>
            <Keycaps caps={["↵"]} />
          </div>
        </div>
      </div>
    </div>
  );
}
