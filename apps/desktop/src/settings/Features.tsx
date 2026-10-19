import { useCallback, useEffect, useState } from "react";
import * as api from "./api";
import "./features.css";
import { SchemaForm } from "./SchemaForm";
import { Toggle } from "./Toggle";
import { errorText, type FeatureView, type ModeToggle } from "./types";

/** Features page: mode toggles plus one generated row per feature. Takes no props. */
export function Features() {
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
      <h1 className="s-title">Features</h1>
      <p className="s-desc">Turn modules on or off. Off means fully stopped.</p>
      {errors.page && (
        <div className="s-error" role="alert">
          {errors.page}
        </div>
      )}
      <h2 className="f-group">Modes</h2>
      <div className="s-row">
        <div className="s-row-text">
          <div className="s-row-label">Command</div>
          <div className="s-help" id="f-command-help">
            Command mode is always on.
          </div>
        </div>
        <div className="s-row-control">
          <Toggle
            checked
            label="Command"
            disabled
            onChange={() => {}}
            describedBy="f-command-help"
          />
        </div>
      </div>
      {modeRow("Find", "find")}
      {modeRow("Query", "query")}

      <h2 className="f-group">Features</h2>
      {features.map((f) => (
        <div key={f.id} className="f-item">
          <div className="s-row">
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
}
