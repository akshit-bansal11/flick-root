import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import type { TrayMenuEvent } from "../ipc";
import { keyWords } from "../shortcuts";
import { applyTheme } from "../theme";
import { buildItems, type MenuAction, reduceMenu } from "./menuReducer";

interface Props {
  event: TrayMenuEvent;
  onRun: (action: MenuAction) => void;
  onClose: () => void;
}

const itemId = (action: MenuAction) => `tray-${action}`;
const NAME_ID = "tray-name";

/** ARIA key names for the key caps of a chord: ["Ctrl", ","] becomes "Control+Comma". */
const ARIA_KEYS: Record<string, string> = { Ctrl: "Control", ",": "Comma" };
const shortcutOf = (caps: string[]) => caps.map((c) => ARIA_KEYS[c] ?? c).join("+");

/** Key caps as muted words, "alt space". */
const keyText = (caps: string[]) => keyWords(caps.join("+")).split("+").join(" ");

/** The tray menu: a flat role=menu under a status line. The window's DWM acrylic is the base. */
export function TrayMenu({ event, onRun, onClose }: Props) {
  const { hotkey, update, appVersion } = event;
  const items = useMemo(() => buildItems({ hotkey, update }), [hotkey, update]);
  const [selected, setSelected] = useState(0);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    applyTheme(document.documentElement, event.appearance?.active ?? null);
  }, [event.appearance]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { state, effects } = reduceMenu({ items, selected }, { type: "key", key: e.key });
      if (state.selected === selected && effects.length === 0 && e.key !== "Tab") return;
      e.preventDefault();
      setSelected(state.selected);
      for (const fx of effects) {
        if (fx.type === "run") onRun(fx.action);
        else onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items, selected, onRun, onClose]);

  useEffect(() => {
    buttons.current[selected]?.focus();
  }, [selected]);

  return (
    <div className="tray-menu">
      <div className="tray-head">
        <span className="tray-dot" aria-hidden="true">
          ●
        </span>
        <span className="tray-name" id={NAME_ID}>
          {APP_DISPLAY_NAME}
        </span>
        <span className="tray-ver">v{appVersion}</span>
        {update && <span className="tray-update">update {update.version} available</span>}
      </div>
      <div className="tray-list" role="menu" aria-labelledby={NAME_ID} tabIndex={-1}>
        {items.map((item, i) => (
          <Fragment key={item.action}>
            {item.action === "quit" && <hr className="tray-sep" />}
            <button
              type="button"
              id={itemId(item.action)}
              role="menuitem"
              className={i === selected ? "tray-item sel" : "tray-item"}
              aria-keyshortcuts={item.caps.length > 0 ? shortcutOf(item.caps) : undefined}
              tabIndex={i === selected ? 0 : -1}
              ref={(el) => {
                buttons.current[i] = el;
              }}
              onMouseEnter={() => setSelected(i)}
              onClick={() => onRun(item.action)}
            >
              <span className="tray-label">{item.label}</span>
              {item.caps.length > 0 && (
                <span className="tray-key" aria-hidden="true">
                  {keyText(item.caps)}
                </span>
              )}
            </button>
          </Fragment>
        ))}
      </div>
    </div>
  );
}
