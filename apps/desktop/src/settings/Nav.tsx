import { type KeyboardEvent, useRef } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { Lens, useLens } from "../Lens";

export const PAGES = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "features", label: "Features" },
  { id: "providers", label: "AI providers" },
  { id: "engine", label: "Decision engine" },
  { id: "integrations", label: "Integrations" },
  { id: "about", label: "About" },
] as const;
export type PageId = (typeof PAGES)[number]["id"];

interface Props {
  active: PageId;
  onSelect: (id: PageId) => void;
  /** "n/m on" text for the Features item, once known. */
  featuresCount: string | null;
}

const itemId = (id: PageId) => `nav-${id}`;

export function Nav({ active, onSelect, featuresCount }: Props) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const list = useRef<HTMLDivElement>(null);
  const lens = useRef<HTMLDivElement>(null);
  // One glass lens glides to the active item; it is placed without animation the first time.
  useLens(list, lens, itemId(active), []);
  const focusAt = (i: number) => refs.current[(i + PAGES.length) % PAGES.length]?.focus();
  const onKeyDown = (e: KeyboardEvent, i: number) => {
    if (e.key === "ArrowDown") focusAt(i + 1);
    else if (e.key === "ArrowUp") focusAt(i - 1);
    else if (e.key === "Home") focusAt(0);
    else if (e.key === "End") focusAt(PAGES.length - 1);
    else return;
    e.preventDefault();
  };
  // Buttons activate on Enter/Space natively; arrows move focus, roving tabindex keeps one tab stop.
  return (
    <nav className="s-nav" aria-label="Settings">
      <div className="s-brand">{APP_DISPLAY_NAME}</div>
      <div ref={list} className="s-nav-list">
        <Lens ref={lens} />
        {PAGES.map((p, i) => (
          <button
            key={p.id}
            id={itemId(p.id)}
            type="button"
            ref={(el) => {
              refs.current[i] = el;
            }}
            className="s-nav-item"
            tabIndex={p.id === active ? 0 : -1}
            aria-current={p.id === active ? "page" : undefined}
            onClick={() => onSelect(p.id)}
            onFocus={() => onSelect(p.id)}
            onKeyDown={(e) => onKeyDown(e, i)}
          >
            <span>{p.label}</span>
            {p.id === "features" && featuresCount && (
              <span className="s-nav-count">{featuresCount}</span>
            )}
          </button>
        ))}
      </div>
    </nav>
  );
}
