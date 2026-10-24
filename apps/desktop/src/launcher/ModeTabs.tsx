import type { LaunchMode } from "../types";
import { MODES } from "./reducer";

interface Props {
  modes: Exclude<LaunchMode, "none">[];
  mode: LaunchMode;
  onSelect: (mode: LaunchMode) => void;
}

/** Mode tab strip: `all` plus one tab per enabled prefix; selecting a tab enters that mode. */
export function ModeTabs({ modes, mode, onSelect }: Props) {
  return (
    <div className="tabs" role="tablist" aria-label="Modes">
      <button
        type="button"
        role="tab"
        aria-selected={mode === "none"}
        tabIndex={-1}
        className="tab"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onSelect("none")}
      >
        all
      </button>
      {modes.map((m) => (
        <button
          key={m}
          type="button"
          role="tab"
          aria-selected={m === mode}
          tabIndex={-1}
          className="tab"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onSelect(m)}
        >
          <b>{MODES[m].prefix}</b>
          {MODES[m].label.toLowerCase()}
        </button>
      ))}
      <span className="bar-note tabs-note">
        <em>ctrl+1</em>…<em>5</em>
      </span>
    </div>
  );
}
