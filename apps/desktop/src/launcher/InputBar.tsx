import type { Ref } from "react";
import { Keycaps } from "../Keycaps";
import { caps } from "../shortcuts";
import type { LaunchMode } from "../types";
import { useExit } from "../useMotion";
import { MODES } from "./reducer";

const NONE_PLACEHOLDER = `Search apps and commands, or type ${Object.values(MODES)
  .map((m) => m.prefix)
  .join(" ")}`;

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
  const chip = useExit(def);
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
      {chip.shown && (
        <span
          className={chip.leaving ? "chip out" : "chip"}
          data-testid={chip.leaving ? undefined : "mode-chip"}
          aria-hidden={chip.leaving || undefined}
          onAnimationEnd={chip.leaving ? chip.onEnd : undefined}
        >
          <span className="glyph" aria-hidden="true">
            {chip.shown.prefix}
          </span>
          {chip.shown.label}
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
      {mode === "none" ? (
        <span className="bar-hint">
          <Keycaps caps={caps("openSettings")} /> Settings
        </span>
      ) : (
        value !== "" && (
          <span className="bar-hint">
            <Keycaps caps={caps("back")} /> Clear
          </span>
        )
      )}
    </div>
  );
}
