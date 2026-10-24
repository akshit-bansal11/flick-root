import { Fragment, useRef } from "react";
import { Lens, useLens } from "../Lens";
import type { Item, LaunchMode } from "../types";
import { groupLabel, HOME_GROUP } from "./kinds";
import { ResultRow } from "./ResultRow";
import { MODES } from "./reducer";

export const LISTBOX_ID = "launcher-list";
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
                  {label === HOME_GROUP ? `# ${label}` : label}
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

interface TabsProps {
  modes: Exclude<LaunchMode, "none">[];
  mode: LaunchMode;
  onSelect: (mode: LaunchMode) => void;
}

/** Mode tab strip: `all` plus one tab per enabled prefix; selecting a tab enters that mode. */
export function ModeTabs({ modes, mode, onSelect }: TabsProps) {
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
    </div>
  );
}
