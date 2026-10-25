"use client";

import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { filter, type Item, type Mode, parse, PREFIX, PRIMARY, SCRIPT, TABS } from "./demoData";

interface Group {
  name: string;
  rows: { item: Item; index: number }[];
}

/** Folds the flat result list into consecutive groups, keeping each row's flat index. */
function groupItems(items: Item[]): Group[] {
  const groups: Group[] = [];
  items.forEach((item, index) => {
    const last = groups[groups.length - 1];
    if (last && last.name === item.group) last.rows.push({ item, index });
    else groups.push({ name: item.group, rows: [{ item, index }] });
  });
  return groups;
}

export function LauncherDemo() {
  const [mode, setMode] = useState<Mode>("none");
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const engaged = useRef(false);
  const items = filter(mode, q);
  const active = Math.min(sel, Math.max(items.length - 1, 0));
  const current: Item | undefined = items[active];
  const chip = Object.keys(PREFIX).find((k) => PREFIX[k] === mode);

  // Autoplay: type scripted examples while idle; stops for good on focus or reduced motion.
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let alive = true;
    const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
    const show = (text: string) => {
      const [m, rest] = parse(text);
      setMode(m);
      setQ(rest);
      setSel(0);
    };
    (async () => {
      await wait(1200);
      for (let i = 0; alive; i = (i + 1) % SCRIPT.length) {
        const text = SCRIPT[i];
        for (let c = 1; c <= text.length; c++) {
          if (!alive || engaged.current) return;
          show(text.slice(0, c));
          await wait(90);
        }
        await wait(1400);
        if (!alive || engaged.current) return;
        show("");
        await wait(500);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const onChange = (v: string) => {
    engaged.current = true;
    const m = mode === "none" && q === "" && v.length === 1 ? PREFIX[v] : undefined;
    if (m) {
      setMode(m);
      setQ("");
    } else setQ(v);
    setSel(0);
  };

  const pickTab = (m: Mode) => {
    engaged.current = true;
    setMode(m);
    setQ("");
    setSel(0);
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Backspace" && q === "" && mode !== "none") {
      e.preventDefault();
      setMode("none");
      setSel(0);
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && items.length) {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      setSel((active + d + items.length) % items.length);
    }
  };

  return (
    <div className="demo">
      <div className="demo-bar">
        <span className="demo-prompt" aria-hidden="true">
          {chip ?? "›"}
        </span>
        <input
          className="demo-input"
          role="combobox"
          aria-label="Launcher demo input"
          aria-expanded="true"
          aria-controls="demo-list"
          aria-activedescendant={current ? `demo-opt-${active}` : undefined}
          placeholder="Type to search…"
          value={q}
          onFocus={() => {
            engaged.current = true;
          }}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKey}
          autoComplete="off"
          spellCheck={false}
        />
      </div>
      <div className="demo-tabs" role="toolbar" aria-label="Modes">
        {TABS.map((t) => (
          <button
            key={t.label}
            type="button"
            className="demo-tab"
            aria-pressed={t.mode === mode}
            onClick={() => pickTab(t.mode)}
          >
            {t.key && <b aria-hidden="true">{t.key}</b>}
            {t.label}
          </button>
        ))}
      </div>
      <div className="demo-split">
        <div id="demo-list" role="listbox" aria-label="Results" className="demo-left">
          {groupItems(items).map((g) => (
            <div key={g.name} role="group" aria-label={g.name}>
              <div className="demo-group" aria-hidden="true">
                # {g.name}
              </div>
              {g.rows.map(({ item, index }) => (
                <div
                  key={item.title}
                  id={`demo-opt-${index}`}
                  role="option"
                  aria-selected={index === active}
                  className="demo-row"
                >
                  <span className="demo-mark" aria-hidden="true">
                    {index === active ? "›" : ""}
                  </span>
                  <span className="demo-title">{item.title}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
        <div className="demo-right">
          {current && (
            <>
              <h3 className="demo-detail-title">{current.title}</h3>
              <dl className="demo-facts">
                <div>
                  <dt>kind</dt>
                  <dd>{current.group}</dd>
                </div>
                <div>
                  <dt>info</dt>
                  <dd>{current.sub}</dd>
                </div>
              </dl>
              <div className="demo-acts">
                <span className="demo-act">
                  {PRIMARY[mode]} <kbd className="demo-key">enter</kbd>
                </span>
                <span className="demo-act">
                  pin <kbd className="demo-key">ctrl p</kbd>
                </span>
              </div>
            </>
          )}
        </div>
      </div>
      <div className="demo-foot">
        <span>
          <kbd className="demo-key">up</kbd> <kbd className="demo-key">down</kbd> select
        </span>
        <span>
          <kbd className="demo-key">backspace</kbd> leave mode
        </span>
        <span>demo only, nothing is sent</span>
      </div>
    </div>
  );
}
