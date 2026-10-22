import { Fragment, useRef } from "react";
import { Keycaps } from "../Keycaps";
import { Lens, useLens } from "../Lens";
import { caps } from "../shortcuts";
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
  /** True while a search is in flight; the empty state waits for its results. */
  pending?: boolean;
  onActivate: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
}

export function Results({ items, selected, query, pending = false, onActivate }: Props) {
  // Window of MAX_ROWS rows that always contains the selection.
  const start = Math.max(0, Math.min(selected - MAX_ROWS + 1, items.length - MAX_ROWS));
  const list = useRef<HTMLDivElement>(null);
  const lens = useRef<HTMLDivElement>(null);
  const visible = items.slice(start, start + MAX_ROWS);
  // One lens glides to the selected row; it moves only when the selection or the rows change.
  useLens(list, lens, items[selected] ? optionId(selected) : undefined, [start, ...visible]);
  return (
    <>
      <div ref={list} id={LISTBOX_ID} role="listbox" aria-label="Results" className="results">
        <Lens ref={lens} />
        {visible.map((item, k) => {
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
                order={k}
                query={query}
                onActivate={onActivate}
              />
            </Fragment>
          );
        })}
      </div>
      {items.length === 0 && !pending && query.trim() !== "" && (
        <div className="empty">No matches</div>
      )}
    </>
  );
}

interface HintProps {
  modes: Exclude<LaunchMode, "none">[];
  selected: number;
  onEnter: (mode: LaunchMode) => void;
}

/** Prefix cheat sheet shown in the default state; each row enters its mode. */
export function PrefixHints({ modes, selected, onEnter }: HintProps) {
  const list = useRef<HTMLDivElement>(null);
  const lens = useRef<HTMLDivElement>(null);
  useLens(list, lens, modes[selected] ? hintId(selected) : undefined, modes);
  return (
    <div ref={list} id={HINTS_ID} className="hints" role="listbox" aria-label="Modes">
      <div className="group" aria-hidden="true">
        Narrow with a prefix
      </div>
      <Lens ref={lens} />
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
          <span className="tile tile-ico tile-glyph">{MODES[m].prefix}</span>
          <span className="hint-text">
            <span className="hint-label">{MODES[m].label}</span>
            <span className="hint-desc">{MODES[m].desc}</span>
          </span>
          <Keycaps caps={caps(`go:${m}`)} />
        </button>
      ))}
    </div>
  );
}
