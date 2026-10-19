import { type CSSProperties, useEffect, useState } from "react";
import {
  applyAppearance,
  FORCED_SCHEME,
  GLASS_MAX,
  GLASS_MIN,
  resolveScheme,
  type Scheme,
  THEMES,
  type ThemeId,
} from "../theme";
import * as api from "./api";
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

  const save = async (patch: Partial<AppearanceView>) => {
    const next = { ...a, ...patch };
    setA(next);
    applyAppearance(document.documentElement, { ...next, glassAvailable: true });
    try {
      setA(await api.setAppearance(patch));
      setError(null);
    } catch (e) {
      setA(a);
      applyAppearance(document.documentElement, { ...a, glassAvailable: true });
      setError(errorText(e));
    }
  };

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

      <div role="radiogroup" aria-label="Theme" className="s-themes">
        {THEMES.map((t) => (
          <label key={t} className="s-theme-card">
            <input
              type="radio"
              name="theme"
              value={t}
              checked={a.theme === t}
              onChange={() => void save({ theme: t })}
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
            type="range"
            min={GLASS_MIN}
            max={GLASS_MAX}
            value={a.glass}
            disabled={a.theme === "solid"}
            onChange={(e) => void save({ glass: Number(e.target.value) })}
          />
          <output htmlFor="s-glass">{a.glass}%</output>
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
        <div role="radiogroup" aria-labelledby="s-scheme-label" className="s-seg">
          {SCHEMES.map((s) => (
            <label key={s.id} className="s-seg-item">
              <input
                type="radio"
                name="scheme"
                value={s.id}
                checked={a.scheme === s.id}
                disabled={!!forced}
                onChange={() => void save({ scheme: s.id })}
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
