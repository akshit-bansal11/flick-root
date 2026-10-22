import { type CSSProperties, useEffect, useRef, useState } from "react";
import { Lens } from "../Lens";
import {
  applyAppearance,
  clampGlass,
  FORCED_SCHEME,
  GLASS_MAX,
  GLASS_MIN,
  resolveScheme,
  type Scheme,
  THEMES,
  type ThemeId,
} from "../theme";
import * as api from "./api";
import { AppearancePreview } from "./AppearancePreview";
import { type AppearanceView, errorText } from "./types";

const LABELS: Record<ThemeId, string> = {
  glass: "Glass",
  frost: "Frost",
  midnight: "Midnight",
  aurora: "Aurora",
  ember: "Ember",
  solid: "Solid",
};
const SCHEMES: { id: Scheme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
];

/** Mini panel drawn from one theme's tokens. */
function Swatch({ theme, scheme, glass }: { theme: ThemeId; scheme: Scheme; glass: number }) {
  const dark = matchMedia("(prefers-color-scheme: dark)").matches;
  return (
    <span
      className="s-swatch"
      data-theme={theme}
      data-scheme={resolveScheme({ theme, glass, scheme }, dark)}
      style={{ "--l-glass-alpha": `${glass}%` } as CSSProperties}
      aria-hidden="true"
    >
      <span className="s-swatch-dot" />
      <span className="s-swatch-line" />
      <span className="s-swatch-line s-swatch-line-short" />
    </span>
  );
}

/** Settings > Appearance: theme cards, glass strength and colour scheme. Each change saves at once. */
export function Appearance() {
  const [a, setA] = useState<AppearanceView | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Slider value while dragging; null when idle. Saved on release. */
  const [drag, setDrag] = useState<number | null>(null);
  /** Saves run one after another so a slow reply never lands after a newer one. */
  const chain = useRef<Promise<void>>(Promise.resolve());
  /** The segmented control's lens is placed by CSS (`--i`); the ref only satisfies `Lens`. */
  const lens = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api
      .getAppearance()
      .then(setA)
      .catch((e: unknown) => setError(errorText(e)));
  }, []);

  if (!a) {
    return (
      <p className="s-help" role={error ? "alert" : undefined}>
        {error ?? "Loading…"}
      </p>
    );
  }

  const save = (patch: Partial<AppearanceView>) => {
    const prev = a;
    const next = { ...a, ...patch };
    setA(next);
    applyAppearance(document.documentElement, { ...next, glassAvailable: true });
    chain.current = chain.current.then(async () => {
      try {
        setA(await api.setAppearance(patch));
        setError(null);
      } catch (e) {
        setA(prev);
        applyAppearance(document.documentElement, { ...prev, glassAvailable: true });
        setError(errorText(e));
      }
    });
  };

  const stored = clampGlass(a.glass);
  const commitGlass = () => {
    if (drag === null) return;
    setDrag(null);
    if (drag !== stored) save({ glass: drag });
  };
  const glass = drag ?? stored;

  const forced = FORCED_SCHEME[a.theme];
  const reducedTransparency = matchMedia("(prefers-reduced-transparency: reduce)").matches;

  return (
    <section>
      <h1 className="s-title">Appearance</h1>
      <p className="s-desc">Theme, glass strength and colour scheme for the launcher.</p>
      {error && (
        <div className="s-error" role="alert">
          {error}
        </div>
      )}

      <AppearancePreview />

      <div role="radiogroup" aria-label="Theme" className="s-themes">
        {THEMES.map((t) => (
          <label key={t} className="s-theme-card">
            <input
              type="radio"
              name="theme"
              value={t}
              checked={a.theme === t}
              onChange={() => save({ theme: t })}
            />
            <Swatch theme={t} scheme={a.scheme} glass={a.glass} />
            <span className="s-theme-name">{LABELS[t]}</span>
          </label>
        ))}
      </div>

      <div className="s-row">
        <div className="s-row-text">
          <label className="s-row-label" htmlFor="s-glass">
            Glass
          </label>
          <div className="s-help">How strongly the panel tints what is behind it.</div>
        </div>
        <div className="s-row-control">
          <input
            id="s-glass"
            className="s-range"
            ref={(el) => {
              // --val is the fill stop of the track, as a percentage of the range.
              const pct = ((glass - GLASS_MIN) / (GLASS_MAX - GLASS_MIN)) * 100;
              el?.style.setProperty("--val", `${pct}%`);
            }}
            type="range"
            min={GLASS_MIN}
            max={GLASS_MAX}
            value={glass}
            disabled={a.theme === "solid"}
            onChange={(e) => {
              const v = Number(e.target.value);
              setDrag(v);
              applyAppearance(document.documentElement, { ...a, glass: v, glassAvailable: true });
            }}
            onPointerUp={commitGlass}
            onKeyUp={commitGlass}
            onBlur={commitGlass}
          />
          <output className="s-val" htmlFor="s-glass">
            {glass}%
          </output>
        </div>
      </div>

      <div className="s-row">
        <div className="s-row-text">
          <div className="s-row-label" id="s-scheme-label">
            Colour scheme
          </div>
          {forced && (
            <div className="s-help">
              {LABELS[a.theme]} is always {forced}.
            </div>
          )}
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
                disabled={!!forced}
                onChange={() => save({ scheme: s.id })}
              />
              <span>{s.label}</span>
            </label>
          ))}
        </div>
      </div>

      {reducedTransparency && (
        <p className="s-help">
          Windows transparency effects are off, so the launcher shows an opaque panel.
        </p>
      )}
    </section>
  );
}
