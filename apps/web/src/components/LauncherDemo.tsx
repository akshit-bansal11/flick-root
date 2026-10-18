"use client";

import { motion } from "motion/react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { filter, MODES, type Mode, PREFIX, SCRIPT } from "./demoData";

export function LauncherDemo() {
  const [mode, setMode] = useState<Mode>("Command");
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const engaged = useRef(false);
  const items = filter(mode, q);
  const active = Math.min(sel, Math.max(items.length - 1, 0));

  // Autoplay: type scripted examples while idle; stops for good on focus or reduced motion.
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let alive = true;
    const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
    (async () => {
      await wait(1200);
      for (let i = 0; alive; i = (i + 1) % SCRIPT.length) {
        const text = SCRIPT[i];
        for (let c = 1; c <= text.length; c++) {
          if (!alive || engaged.current) return;
          setQ(text.slice(0, c));
          await wait(90);
        }
        await wait(1400);
        if (!alive || engaged.current) return;
        setQ("");
        await wait(500);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const onChange = (v: string) => {
    engaged.current = true;
    const m = q === "" && v.length === 1 ? PREFIX[v] : undefined;
    if (m) {
      setMode(m);
      setQ("");
    } else setQ(v);
    setSel(0);
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      const d = e.shiftKey ? MODES.length - 1 : 1;
      setMode(MODES[(MODES.indexOf(mode) + d) % MODES.length]);
      setSel(0);
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && items.length) {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      setSel((active + d + items.length) % items.length);
    }
  };

  return (
    <motion.div
      className="demo"
      initial={{ opacity: 0, scale: 0.98 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.1 }}
    >
      <div className="demo-bar">
        <span className="demo-mode">{mode}</span>
        <input
          className="demo-input"
          role="combobox"
          aria-label="Launcher demo input"
          aria-expanded="true"
          aria-controls="demo-list"
          aria-activedescendant={items.length ? `demo-opt-${active}` : undefined}
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
      <div id="demo-list" role="listbox" aria-label="Results" className="demo-list">
        {items.map((it, i) => (
          <div
            key={it.title}
            id={`demo-opt-${i}`}
            role="option"
            tabIndex={-1}
            aria-selected={i === active}
            className="demo-row"
          >
            <span className="demo-tile" aria-hidden="true">
              {it.title[0]}
            </span>
            <span>{it.title}</span>
            <span className="demo-sub">{it.sub}</span>
          </div>
        ))}
      </div>
      <div className="demo-hint">
        <span>Tab: switch mode</span>
        <span>Up/Down: select</span>
        <span>{">"} / ? prefixes</span>
        <span>Demo only, nothing is sent</span>
      </div>
    </motion.div>
  );
}
