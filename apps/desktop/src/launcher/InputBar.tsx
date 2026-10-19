import type { Ref } from "react";
import type { LaunchMode } from "../types";
import { MODES } from "./reducer";

const NONE_PLACEHOLDER = "Type > ! / or ? to start";

interface Props {
  mode: LaunchMode;
  value: string;
  /** Id of the list the input controls, and the highlighted descendant in it. */
  listId: string;
  activeId: string | undefined;
  onChange: (text: string) => void;
  inputRef: Ref<HTMLInputElement>;
}

export function InputBar({ mode, value, listId, activeId, onChange, inputRef }: Props) {
  const def = mode === "none" ? null : MODES[mode];
  return (
    <div className="inputbar">
      <svg
        className="input-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="11" cy="11" r="7" />
        <path d="m20 20-3.5-3.5" />
      </svg>
      {def && (
        <span className="chip" data-testid="mode-chip">
          {def.label}
        </span>
      )}
      <input
        ref={inputRef}
        className="input"
        type="text"
        role="combobox"
        aria-label={def ? `${def.label} search` : "Search"}
        aria-expanded={activeId !== undefined}
        aria-controls={listId}
        aria-activedescendant={activeId}
        autoComplete="off"
        spellCheck={false}
        placeholder={def ? def.placeholder : NONE_PLACEHOLDER}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
