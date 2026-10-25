import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { type ActiveTheme, applyTheme, type ThemeView } from "../theme";
import { ThemeMini, themeVars } from "./ThemeMini";
import {
  chipCounts,
  initPicker,
  PICKER_CHIPS,
  type PickerChip,
  type PickerEffect,
  type PickerEvent,
  type PickerKey,
  type PickerState,
  reducePicker,
  slotOf,
  type ThemeSlot,
  visibleThemes,
} from "./themePicker";
import type { AppearancePatch } from "./types";
import "./themes.css";

const CHIP_LABEL: Record<PickerChip, string> = {
  all: "all",
  dark: "dark",
  light: "light",
  flick: "flick",
  ide: "ide ports",
  imported: "imported",
};
const SOURCE_TAG: Record<ThemeView["source"], string> = {
  builtin: "flick",
  port: "ide",
  imported: "imported",
};
/** Keys the filter input hands to the reducer (Home and End stay caret keys in a text field). */
const INPUT_KEYS: readonly string[] = ["ArrowUp", "ArrowDown", "Enter"];

const ratio = (n: number) => `${n.toFixed(1)}:1`;
const asKey = (key: string): PickerKey | null =>
  key === "ArrowUp" || key === "ArrowDown" || key === "Enter" || key === "Escape" ? key : null;

interface Props {
  /** Loaded once by the page; arrow-key previews read this list and never call IPC. */
  themes: ThemeView[];
  /** The slot being changed; opens pre-filtered to its variant with the saved theme selected. */
  slot: ThemeSlot;
  darkId: string;
  lightId: string;
  /** The saved theme, restored by Esc and when nothing is selected. */
  saved: ActiveTheme | null;
  /** Saves a patch; resolves true when it was stored. */
  onSave: (patch: AppearancePatch) => Promise<boolean>;
  onClose: () => void;
}

/** Filter, chips and a listbox of themes with a live preview of the highlighted one. */
export function ThemeList({ themes, slot, darkId, lightId, saved, onSave, onClose }: Props) {
  const isDark = slot === "darkTheme";
  const [pk, setPk] = useState<PickerState>(() =>
    initPicker(themes, isDark ? "dark" : "light", isDark ? darkId : lightId),
  );
  const listId = useId();
  const input = useRef<HTMLInputElement>(null);
  const wrap = useRef<HTMLFieldSetElement>(null);
  const root = document.documentElement;

  useEffect(() => {
    input.current?.focus();
  }, []);

  const optionId = (id: string) => `${listId}-${id}`;
  const preview = (id: string) => {
    const t = themes.find((x) => x.id === id);
    if (t) applyTheme(root, { id: t.id, dark: t.dark, roles: t.roles });
  };

  const run = (effect: PickerEffect) => {
    switch (effect.type) {
      case "preview":
        return preview(effect.id);
      case "revert":
        return applyTheme(root, saved);
      case "close":
        return onClose();
      case "apply":
        // The page applies the returned theme; a failed save restores the saved one there.
        return void onSave({ [effect.slot]: effect.id }).then((ok) => ok && onClose());
    }
  };

  const dispatch = (event: PickerEvent) => {
    const step = reducePicker(pk, event);
    setPk(step.state);
    for (const e of step.effects) run(e);
  };

  // Keep the highlighted row in view while arrowing.
  useEffect(() => {
    if (!pk.selected) return;
    document.getElementById(optionId(pk.selected))?.scrollIntoView?.({ block: "nearest" });
  });

  // Esc anywhere in the picker reverts and closes; the settings window must not also see it.
  // Native listener: the wrapper is not itself interactive, its controls are.
  const latest = useRef(dispatch);
  latest.current = dispatch;
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const onEscape = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      latest.current({ type: "key", key: "Escape" });
    };
    el.addEventListener("keydown", onEscape);
    return () => el.removeEventListener("keydown", onEscape);
  }, []);

  const onInputKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const key = INPUT_KEYS.includes(e.key) ? asKey(e.key) : null;
    if (!key) return;
    e.preventDefault();
    dispatch({ type: "key", key });
  };

  const list = visibleThemes(pk);
  const counts = chipCounts(pk.themes);
  const sel = pk.themes.find((t) => t.id === pk.selected) ?? null;
  const heading = isDark ? "dark" : "light";

  return (
    <fieldset ref={wrap} className="tp" aria-label={`Choose ${heading} theme`}>
      <div className="sec"># choose {heading} theme</div>
      <div className="tp-filter">
        <span className="tp-pr" aria-hidden="true">
          ›
        </span>
        <input
          ref={input}
          type="text"
          role="combobox"
          aria-label="Filter themes"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={pk.selected ? optionId(pk.selected) : undefined}
          placeholder="filter themes"
          autoComplete="off"
          spellCheck={false}
          value={pk.query}
          onChange={(e) => dispatch({ type: "query", query: e.target.value })}
          onKeyDown={onInputKey}
        />
        <span className="tp-keys">↑↓ preview · enter apply · esc revert</span>
      </div>
      <div className="tp-chips">
        {PICKER_CHIPS.map((c) => (
          <button
            key={c}
            type="button"
            className="tp-chip"
            aria-pressed={pk.chip === c}
            onClick={() => dispatch({ type: "chip", chip: c })}
          >
            {CHIP_LABEL[c]} {counts[c]}
          </button>
        ))}
      </div>
      <div className="tp-body">
        <div id={listId} className="tp-list" role="listbox" aria-label={`${heading} themes`}>
          {list.length === 0 && <p className="tp-empty">No theme matches.</p>}
          {list.map((t) => {
            const on = t.id === pk.selected;
            return (
              // Keys go through the filter input (aria-activedescendant); click is a pointer shortcut.
              <div
                key={t.id}
                id={optionId(t.id)}
                role="option"
                aria-selected={on}
                tabIndex={-1}
                className="tp-opt"
                style={themeVars(t.roles)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => dispatch({ type: "pick", id: t.id })}
                onDoubleClick={() => dispatch({ type: "key", key: "Enter" })}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && on) dispatch({ type: "key", key: "Enter" });
                  else if (e.key === "Enter" || e.key === " ") {
                    dispatch({ type: "pick", id: t.id });
                  }
                }}
              >
                <span className="tp-mark" aria-hidden="true">
                  {on ? "›" : ""}
                </span>
                <span className="tp-sw" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                </span>
                <b>{t.name}</b>
                <span className="tp-cur">
                  {t.id === darkId || t.id === lightId ? "current" : ""}
                </span>
                <span className="tp-tag">{SOURCE_TAG[t.source]}</span>
              </div>
            );
          })}
        </div>
        {sel && (
          <div className="tp-prev" aria-live="polite">
            <h3>{sel.name}</h3>
            <span className="tp-sub">
              {SOURCE_TAG[sel.source]} · {sel.dark ? "dark" : "light"}
            </span>
            {sel.upstream && <span className="tp-sub">from {sel.upstream}</span>}
            <ThemeMini roles={sel.roles} />
            <dl className="tp-ratios">
              <dt>text</dt>
              <dd>{ratio(sel.ratios.text)}</dd>
              <dt>muted</dt>
              <dd>{ratio(sel.ratios.muted)}</dd>
              <dt>accent</dt>
              <dd>{ratio(sel.ratios.accentText)}</dd>
              <dt>on accent</dt>
              <dd>{ratio(sel.ratios.onAccent)}</dd>
            </dl>
            {sel.adjusted.length > 0 && (
              <p className="tp-fix">raised for contrast: {sel.adjusted.join(", ")}</p>
            )}
            <div className="tp-btns">
              <button
                type="button"
                className="btn pri"
                onClick={() => run({ type: "apply", id: sel.id, slot: slotOf(sel) })}
              >
                use as {sel.dark ? "dark" : "light"} theme
              </button>
              <button type="button" className="btn" disabled aria-describedby={`${listId}-imp`}>
                import VS Code theme…
              </button>
              <span id={`${listId}-imp`} className="tp-sub">
                Importing arrives in a later update.
              </span>
            </div>
          </div>
        )}
      </div>
    </fieldset>
  );
}
