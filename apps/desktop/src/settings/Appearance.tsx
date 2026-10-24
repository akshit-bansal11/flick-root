import { useEffect, useRef, useState } from "react";
import { Lens } from "../Lens";
import { applyTheme, type Scheme, type ThemeView } from "../theme";
import * as api from "./api";
import { type AppearancePatch, type AppearanceView, errorText } from "./types";

const SCHEMES: { id: Scheme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
];

const SLOTS = [
  { key: "darkTheme", dark: true, label: "Dark theme", help: "Used when the scheme is dark." },
  { key: "lightTheme", dark: false, label: "Light theme", help: "Used when the scheme is light." },
] as const;

/** Settings > Appearance: colour scheme and the theme for each scheme. Each change saves at once. */
export function Appearance() {
  const [a, setA] = useState<AppearanceView | null>(null);
  const [themes, setThemes] = useState<ThemeView[]>([]);
  const [error, setError] = useState<string | null>(null);
  /** Saves run one after another so a slow reply never lands after a newer one. */
  const chain = useRef<Promise<void>>(Promise.resolve());
  /** The segmented control's lens is placed by CSS (`--i`); the ref only satisfies `Lens`. */
  const lens = useRef<HTMLDivElement>(null);

  useEffect(() => {
    Promise.all([api.getAppearance(), api.listThemes()])
      .then(([view, list]) => {
        setA(view);
        setThemes(list);
      })
      .catch((e: unknown) => setError(errorText(e)));
  }, []);

  if (!a) {
    return (
      <p className="s-help" role={error ? "alert" : undefined}>
        {error ?? "Loading…"}
      </p>
    );
  }

  const save = (patch: AppearancePatch) => {
    const prev = a;
    setA({ ...a, ...patch });
    chain.current = chain.current.then(async () => {
      try {
        const next = await api.setAppearance(patch);
        setA(next);
        applyTheme(document.documentElement, next.active);
        setError(null);
      } catch (e) {
        setA(prev);
        applyTheme(document.documentElement, prev.active);
        setError(errorText(e));
      }
    });
  };

  return (
    <section>
      <h1 className="s-title">Appearance</h1>
      <p className="s-desc">Colour scheme and the theme used for each scheme.</p>
      {error && (
        <div className="s-error" role="alert">
          {error}
        </div>
      )}

      <div className="s-row">
        <div className="s-row-text">
          <div className="s-row-label" id="s-scheme-label">
            Colour scheme
          </div>
        </div>
        <div
          role="radiogroup"
          aria-labelledby="s-scheme-label"
          className="s-seg"
          data-lens="1"
          ref={(el) => {
            // --n and --i place the lens: it is one item wide and moves by whole item widths.
            el?.style.setProperty("--n", String(SCHEMES.length));
            el?.style.setProperty("--i", String(SCHEMES.findIndex((s) => s.id === a.scheme)));
          }}
        >
          <Lens ref={lens} />
          {SCHEMES.map((s) => (
            <label key={s.id} className="s-seg-item">
              <input
                type="radio"
                name="scheme"
                value={s.id}
                checked={a.scheme === s.id}
                onChange={() => save({ scheme: s.id })}
              />
              <span>{s.label}</span>
            </label>
          ))}
        </div>
      </div>

      {SLOTS.map((slot) => (
        <div key={slot.key} className="s-row">
          <div className="s-row-text">
            <label className="s-row-label" htmlFor={`s-${slot.key}`}>
              {slot.label}
            </label>
            <div className="s-help">{slot.help}</div>
          </div>
          <div className="s-row-control">
            <select
              id={`s-${slot.key}`}
              className="s-select"
              value={a[slot.key]}
              onChange={({ target: { value } }) =>
                save(slot.dark ? { darkTheme: value } : { lightTheme: value })
              }
            >
              {themes
                .filter((t) => t.dark === slot.dark)
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
            </select>
          </div>
        </div>
      ))}
    </section>
  );
}
