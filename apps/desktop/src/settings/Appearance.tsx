import { useEffect, useRef, useState } from "react";
import { type ActiveTheme, applyTheme, type Scheme, type ThemeView } from "../theme";
import * as api from "./api";
import { SettingRow } from "./Row";
import { ThemeList } from "./ThemeList";
import type { ThemeSlot } from "./themePicker";
import { type AppearancePatch, type AppearanceView, errorText } from "./types";

const SCHEMES: Scheme[] = ["system", "dark", "light"];
const SCHEME_HINT: Record<Scheme, string> = {
  system: "follows Windows",
  dark: "always dark",
  light: "always light",
};

const nameOf = (themes: ThemeView[], id: string) => themes.find((t) => t.id === id)?.name ?? id;

/** Settings > Appearance: colour scheme, the theme for each scheme, and the theme picker. */
export function Appearance() {
  const [a, setA] = useState<AppearanceView | null>(null);
  const [themes, setThemes] = useState<ThemeView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [picking, setPicking] = useState<ThemeSlot | null>(null);
  /** Saves run one after another so a slow reply never lands after a newer one. */
  const chain = useRef<Promise<void>>(Promise.resolve());

  useEffect(() => {
    Promise.all([api.getAppearance(), api.listThemes()])
      .then(([view, list]) => {
        setA(view);
        setThemes(list);
      })
      .catch((e: unknown) => setError(errorText(e)));
  }, []);

  // Whenever the picker closes or changes slot (Esc, unmount, `change` toggled off, a save),
  // the saved theme is what stays painted: an unapplied preview never outlives its picker.
  const saved = useRef<ActiveTheme | null>(null);
  saved.current = a?.active ?? null;
  useEffect(() => {
    if (!picking) return;
    return () => applyTheme(document.documentElement, saved.current);
  }, [picking]);

  if (!a) {
    return (
      <p className="s-help" role={error ? "alert" : undefined}>
        {error ?? "Loading…"}
      </p>
    );
  }

  /** Saves a patch, applies the returned theme, and resolves true when it was stored. */
  const save = (patch: AppearancePatch): Promise<boolean> => {
    const prev = a;
    setA({ ...a, ...patch });
    const done = chain.current.then(async () => {
      try {
        const next = await api.setAppearance(patch);
        setA(next);
        applyTheme(document.documentElement, next.active);
        setError(null);
        return true;
      } catch (e) {
        setA(prev);
        applyTheme(document.documentElement, prev.active);
        setError(errorText(e));
        return false;
      }
    });
    chain.current = done.then(() => undefined);
    return done;
  };

  const slotRow = (slot: ThemeSlot, label: string, id: string) => (
    <SettingRow
      label={label}
      value={nameOf(themes, id)}
      preview={{
        title: label,
        body: `The theme used when the scheme is ${slot === "darkTheme" ? "dark" : "light"}. Change opens a filterable list with a live preview.`,
      }}
    >
      <button
        type="button"
        className="btn"
        aria-expanded={picking === slot}
        onClick={() => setPicking(picking === slot ? null : slot)}
      >
        change
      </button>
    </SettingRow>
  );

  return (
    <section>
      <h1 className="s-title">Appearance</h1>
      <p className="s-desc">{themes.length} themes. Pick one, or one for dark and one for light.</p>
      {error && (
        <div className="s-error" role="alert">
          {error}
        </div>
      )}

      <SettingRow
        label="scheme"
        hint={SCHEME_HINT[a.scheme]}
        preview={{
          title: "Colour scheme",
          body: "System follows the Windows setting; dark and light force one of the two themes below.",
        }}
      >
        <fieldset className="tg" aria-label="Colour scheme">
          {SCHEMES.map((s) => (
            <button
              key={s}
              type="button"
              aria-pressed={a.scheme === s}
              onClick={() => {
                if (a.scheme !== s) void save({ scheme: s });
              }}
            >
              {s}
            </button>
          ))}
        </fieldset>
      </SettingRow>
      {slotRow("darkTheme", "dark theme", a.darkTheme)}
      {slotRow("lightTheme", "light theme", a.lightTheme)}

      {picking && (
        <ThemeList
          key={picking}
          themes={themes}
          slot={picking}
          darkId={a.darkTheme}
          lightId={a.lightTheme}
          saved={a.active}
          onSave={save}
          onClose={() => setPicking(null)}
        />
      )}
    </section>
  );
}
