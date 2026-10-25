import { keyWords, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { MODES } from "./reducer";

interface Props {
  modes: Exclude<LaunchMode, "none">[];
  mode: LaunchMode;
  onSelect: (mode: LaunchMode) => void;
}

/** Mode tab strip: `all` plus one tab per enabled prefix; selecting a tab enters that mode. */
export function ModeTabs({ modes, mode, onSelect }: Props) {
  const chords = modes.map((m) => keyWords(shortcut(`go:${m}`).keys));
  const first = chords[0];
  const last = chords[chords.length - 1];
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
      {first && last && (
        <span className="bar-note tabs-note">
          <em>{first}</em>
          {last !== first && (
            <>
              …<em>{last.split("+").pop()}</em>
            </>
          )}
        </span>
      )}
    </div>
  );
}
