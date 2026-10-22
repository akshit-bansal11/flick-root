import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import type { TrayMenuEvent } from "../ipc";
import { Keycaps } from "../Keycaps";
import { Lens, useLens } from "../Lens";
import { applyAppearance, DEFAULT_APPEARANCE } from "../theme";
import { buildItems, type MenuAction, reduceMenu } from "./menuReducer";

interface Props {
  event: TrayMenuEvent;
  onRun: (action: MenuAction) => void;
  onClose: () => void;
}

const itemId = (action: MenuAction) => `tray-${action}`;

/** The tray menu: a glass role=menu with a roving selection lens. Base glass is the window's DWM acrylic. */
export function TrayMenu({ event, onRun, onClose }: Props) {
  const { hotkey, update, appVersion } = event;
  const items = useMemo(() => buildItems({ hotkey, update }), [hotkey, update]);
  const [selected, setSelected] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const lens = useRef<HTMLDivElement>(null);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    applyAppearance(document.documentElement, {
      ...(event.appearance ?? DEFAULT_APPEARANCE),
      glassAvailable: event.glass !== false,
    });
  }, [event.appearance, event.glass]);

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

  const current = items[selected];
  useLens(list, lens, current ? itemId(current.action) : undefined, [items]);

  return (
    <div className="tray-menu" role="menu" aria-label={APP_DISPLAY_NAME} tabIndex={-1}>
      <div className="tray-head">
        <span className="tray-mark" aria-hidden="true">
          {APP_DISPLAY_NAME.charAt(0)}
        </span>
        <span className="tray-name">{APP_DISPLAY_NAME}</span>
        <span className="tray-ver">v{appVersion}</span>
      </div>
      <div className="tray-list" ref={list}>
        <Lens ref={lens} />
        {items.map((item, i) => (
          <Fragment key={item.action}>
            {item.action === "quit" && <hr className="tray-sep" />}
            <button
              type="button"
              id={itemId(item.action)}
              role="menuitem"
              className="tray-item"
              tabIndex={i === selected ? 0 : -1}
              ref={(el) => {
                buttons.current[i] = el;
              }}
              onMouseEnter={() => setSelected(i)}
              onClick={() => onRun(item.action)}
            >
              <span className="tray-label">{item.label}</span>
              {item.caps.length > 0 && <Keycaps caps={item.caps} />}
            </button>
          </Fragment>
        ))}
      </div>
    </div>
  );
}
