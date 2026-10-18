import type { Ref } from "react";
import type { Mode } from "../types";
import { LISTBOX_ID, optionId } from "./Results";

const COPY: Record<Mode, { label: string; placeholder: string }> = {
  command: { label: "Command", placeholder: "Open an app or run a command" },
  find: { label: "Find", placeholder: "Find a file or folder" },
  query: { label: "Query", placeholder: "Ask anything" },
};

interface Props {
  mode: Mode;
  value: string;
  hasItems: boolean;
  selected: number;
  onChange: (text: string) => void;
  inputRef: Ref<HTMLInputElement>;
}

export function InputBar({ mode, value, hasItems, selected, onChange, inputRef }: Props) {
  const { label, placeholder } = COPY[mode];
  return (
    <div className="inputbar">
      <input
        ref={inputRef}
        className="input"
        type="text"
        role="combobox"
        aria-label={`${label} search`}
        aria-expanded={hasItems}
        aria-controls={LISTBOX_ID}
        aria-activedescendant={hasItems ? optionId(selected) : undefined}
        autoComplete="off"
        spellCheck={false}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <span className="hint" data-testid="mode-hint">
        {label} · Tab
      </span>
    </div>
  );
}
