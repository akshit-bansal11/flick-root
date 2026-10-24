import type { Ref } from "react";
import { keyWords, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { MODES } from "./reducer";

const NONE_PLACEHOLDER = `Search apps and commands, or type ${Object.values(MODES)
  .map((m) => m.prefix)
  .join(" ")}`;

/** A shortcut as the bar shows it in words: `ctrl+,`. */
const chord = (id: string) => keyWords(shortcut(id).keys);

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
      <span className="prompt" data-testid="prompt" aria-hidden="true">
        {def ? def.prefix : "›"}
      </span>
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
      {mode === "none" ? (
        <span className="bar-note">
          <em>{chord("openSettings")}</em> settings
        </span>
      ) : (
        value !== "" && (
          <span className="bar-note">
            <em>{chord("back")}</em> clear
          </span>
        )
      )}
    </div>
  );
}
