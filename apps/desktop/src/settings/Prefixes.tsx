import { useCallback, useEffect, useState } from "react";
import { MODES } from "../launcher/reducer";
import * as api from "./api";
import "./features.css";
import { OTHER_GROUP, PREFIX_GROUPS, type PrefixGroup } from "./prefixGroups";
import { SchemaForm } from "./SchemaForm";
import { Toggle } from "./Toggle";
import { errorText, type FeatureView, type ModeToggle } from "./types";

/** Settings > Prefixes: one section per launcher prefix, with its mode switch and its modules. Takes no props. */
export function Prefixes() {
  const [features, setFeatures] = useState<FeatureView[]>([]);
  const [modes, setModes] = useState({ find: false, query: false });
  const [open, setOpen] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string | null>>({});
  const err = useCallback(
    (k: string, e: unknown) => setErrors((p) => ({ ...p, [k]: e ? errorText(e) : null })),
    [],
  );

  const refresh = useCallback(async () => {
    try {
      const [list, s] = await Promise.all([api.listFeatures(), api.getSettings()]);
      setFeatures(list.features);
      setModes({ find: s.findEnabled, query: s.queryEnabled });
    } catch (e) {
      err("page", e);
    }
  }, [err]);

  useEffect(() => {
    void refresh();
    const un = api.onFeaturesChanged(() => void refresh());
    return () => void un.then((f) => f());
  }, [refresh]);

  const patch = (id: string, p: Partial<FeatureView>) =>
    setFeatures((l) => l.map((x) => (x.id === id ? { ...x, ...p } : x)));

  const toggleFeature = async (f: FeatureView, enabled: boolean) => {
    patch(f.id, { enabled });
    try {
      const v = await api.setFeatureEnabled(f.id, enabled);
      setFeatures((l) => l.map((x) => (x.id === f.id ? v : x)));
      err(f.id, null);
    } catch (e) {
      patch(f.id, { enabled: f.enabled });
      err(f.id, e);
    }
  };

  const toggleMode = async (mode: ModeToggle, enabled: boolean) => {
    setModes((m) => ({ ...m, [mode]: enabled }));
    try {
      await api.setModeEnabled(mode, enabled);
      err(mode, null);
    } catch (e) {
      setModes((m) => ({ ...m, [mode]: !enabled }));
      err(mode, e);
    }
  };

  const saveOptions = async (id: string, options: Record<string, unknown>) => {
    const v = await api.setFeatureOptions(id, options);
    setFeatures((l) => l.map((x) => (x.id === id ? v : x)));
  };

  const grouped = new Set(PREFIX_GROUPS.flatMap((g) => g.features));
  const other = features.filter((f) => !grouped.has(f.id));
  const groups = other.length > 0 ? [...PREFIX_GROUPS, OTHER_GROUP] : PREFIX_GROUPS;
  const itemsOf = (g: PrefixGroup) =>
    g.id === OTHER_GROUP.id
      ? other
      : g.features.flatMap((id) => features.find((f) => f.id === id) ?? []);

  const modeRow = (label: string, key: ModeToggle) => (
    <div className="s-row">
      <div className="s-row-text">
        <div className="s-row-label">{label}</div>
        {errors[key] && (
          <div className="s-error" role="alert">
            {errors[key]}
          </div>
        )}
      </div>
      <div className="s-row-control">
        <Toggle checked={modes[key]} label={label} onChange={(v) => void toggleMode(key, v)} />
      </div>
    </div>
  );

  return (
    <section>
      <h1 className="s-title">Prefixes</h1>
      <p className="s-desc">
        Each prefix searches its own modules. Turn a module off and it stops completely; a prefix
        with nothing on is hidden from the launcher.
      </p>
      {errors.page && (
        <div className="s-error" role="alert">
          {errors.page}
        </div>
      )}
      {groups.map((g) => {
        const items = itemsOf(g);
        if (items.length === 0 && !g.toggle) return null;
        const def = g.mode ? MODES[g.mode] : null;
        return (
          <section key={g.id} aria-labelledby={`f-head-${g.id}`}>
            <div className="f-head">
              {def && (
                <span className="f-glyph" aria-hidden="true">
                  {def.prefix}
                </span>
              )}
              <h2 className="f-title" id={`f-head-${g.id}`}>
                {def?.label ?? g.title}
              </h2>
              <span className="f-sub">{def?.desc ?? g.desc}</span>
            </div>
            {g.toggle && modeRow(g.toggle.label, g.toggle.key)}
            {items.map((f) => (
              <div key={f.id} className="s-row f-feature">
                <span className="f-tile" aria-hidden="true">
                  {f.title.charAt(0).toUpperCase()}
                </span>
                <div className="s-row-text f-grow">
                  <div className="s-row-label">
                    {f.title} <span className="f-pill">{f.versionTag}</span>
                  </div>
                  <div className="s-help">{f.description}</div>
                  {errors[f.id] && (
                    <div className="s-error" role="alert">
                      {errors[f.id]}
                    </div>
                  )}
                </div>
                <div className="s-row-control">
                  {f.schema.fields.length > 0 && (
                    <button
                      type="button"
                      className="s-btn"
                      aria-expanded={open === f.id}
                      aria-label={`Options for ${f.title}`}
                      onClick={() => setOpen(open === f.id ? null : f.id)}
                    >
                      Options
                    </button>
                  )}
                  <Toggle
                    checked={f.enabled}
                    label={`Enable ${f.title}`}
                    onChange={(v) => void toggleFeature(f, v)}
                  />
                </div>
                {open === f.id && (
                  <SchemaForm
                    fields={f.schema.fields}
                    options={f.options}
                    onSave={(o) => saveOptions(f.id, o)}
                  />
                )}
              </div>
            ))}
          </section>
        );
      })}
    </section>
  );
}
