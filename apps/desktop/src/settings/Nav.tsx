import { Fragment, type KeyboardEvent, useRef } from "react";

export const PAGES = [
  { id: "general", label: "general", group: "launcher" },
  { id: "appearance", label: "appearance", group: "launcher" },
  { id: "prefixes", label: "prefixes", group: "launcher" },
  { id: "providers", label: "providers", group: "ai" },
  { id: "engine", label: "decision engine", group: "ai" },
  { id: "integrations", label: "integrations", group: "connect" },
  { id: "about", label: "about", group: null },
] as const;
export type PageId = (typeof PAGES)[number]["id"];

interface Props {
  active: PageId;
  onSelect: (id: PageId) => void;
  /** "n/m on" text for the Prefixes item, once known. */
  featuresCount: string | null;
  /** App version for the About item, once known. */
  version: string | null;
}

const itemId = (id: PageId) => `nav-${id}`;

export function Nav({ active, onSelect, featuresCount, version }: Props) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
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
      {PAGES.map((p, i) => (
        <Fragment key={p.id}>
          {p.group && p.group !== PAGES[i - 1]?.group && (
            <div className="s-nav-group">{`# ${p.group}`}</div>
          )}
          <button
            id={itemId(p.id)}
            type="button"
            ref={(el) => {
              refs.current[i] = el;
            }}
            className={p.id === "about" ? "s-nav-item s-nav-end" : "s-nav-item"}
            tabIndex={p.id === active ? 0 : -1}
            aria-current={p.id === active ? "page" : undefined}
            onClick={() => onSelect(p.id)}
            onFocus={() => onSelect(p.id)}
            onKeyDown={(e) => onKeyDown(e, i)}
          >
            <span className="s-nav-mark" aria-hidden="true">›</span>
            <span>{p.id === "about" && version ? `about · v${version}` : p.label}</span>
            {p.id === "prefixes" && featuresCount && (
              <span className="s-nav-count">{featuresCount}</span>
            )}
          </button>
        </Fragment>
      ))}
    </nav>
  );
}
