import { Fragment } from "react";
import type { Item, LaunchMode } from "../types";
import { groupLabel } from "./kinds";
import { ResultRow } from "./ResultRow";
import { MODES } from "./reducer";

export const LISTBOX_ID = "launcher-list";
export const HINTS_ID = "launcher-hints";
export const hintId = (i: number) => `launcher-hint-${i}`;
export const optionId = (i: number) => `launcher-opt-${i}`;
const MAX_ROWS = 8;

interface Props {
  items: Item[];
  selected: number;
  query: string;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

export function Results({ items, selected, query, onActivate }: Props) {
  // Window of MAX_ROWS rows that always contains the selection.
  const start = Math.max(0, Math.min(selected - MAX_ROWS + 1, items.length - MAX_ROWS));
  return (
    <>
      <div id={LISTBOX_ID} role="listbox" aria-label="Results" className="results">
        {items.slice(start, start + MAX_ROWS).map((item, k) => {
          const index = start + k;
          const label = groupLabel(item);
          const prev = items[index - 1];
          return (
            <Fragment key={item.id}>
              {(!prev || groupLabel(prev) !== label) && (
                <div className="group" aria-hidden="true">
                  {label}
                </div>
              )}
              <ResultRow
                item={item}
                index={index}
                id={optionId(index)}
                selected={index === selected}
                query={query}
                onActivate={onActivate}
              />
            </Fragment>
          );
        })}
      </div>
      {items.length === 0 && query.trim() !== "" && <div className="empty">No matches</div>}
    </>
  );
}

interface HintProps {
  modes: Exclude<LaunchMode, "none">[];
  selected: number;
  /** True once the user typed text that matched no prefix. */
  typed: boolean;
  onEnter: (mode: LaunchMode) => void;
}

/** Prefix cheat sheet shown in the default state; each row enters its mode. */
export function PrefixHints({ modes, selected, typed, onEnter }: HintProps) {
  return (
    <div id={HINTS_ID} className="hints" role="listbox" aria-label="Modes">
      {modes.map((m, i) => (
        <button
          key={m}
          id={hintId(i)}
          type="button"
          role="option"
          aria-selected={i === selected}
          tabIndex={-1}
          className={i === selected ? "hint-row sel" : "hint-row"}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onEnter(m)}
        >
          <kbd className="kbd">{MODES[m].prefix}</kbd>
          <span className="hint-label">{MODES[m].label}</span>
          <span className="hint-desc">{MODES[m].desc}</span>
        </button>
      ))}
      {typed && <p className="hint-note">Start with &gt; ! / or ? to search</p>}
    </div>
  );
}
