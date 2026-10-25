import { Fragment, useCallback, useEffect, useState } from "react";
import { MODES } from "../launcher/reducer";
import * as api from "./api";
import "./features.css";
import { OnOff } from "./OnOff";
import { OTHER_GROUP, PREFIX_GROUPS, type PrefixGroup } from "./prefixGroups";
import { SettingRow } from "./Row";
import { SchemaForm } from "./SchemaForm";
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

  const fail = (k: string) =>
    errors[k] && (
      <div className="s-error" role="alert">
        {errors[k]}
      </div>
    );

  const modeRow = (label: string, key: ModeToggle) => (
    <>
      <SettingRow
        label={label}
        hint="switches the whole prefix"
        help="Turn it off and the prefix stops completely; with nothing on it is hidden from the launcher."
        preview={{
          title: `# ${label.toLowerCase()}`,
          body: "Turn it off and the prefix stops completely; with nothing on it is hidden from the launcher.",
        }}
      >
        <OnOff checked={modes[key]} label={label} onChange={(v) => void toggleMode(key, v)} />
      </SettingRow>
      {fail(key)}
    </>
  );

  return (
    <section>
      <h1 className="s-title">Prefixes</h1>
      <p className="s-desc s-prose">
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
            <h2 className="sec" id={`f-head-${g.id}`}>
              {`# ${(def?.label ?? g.title ?? g.id).toLowerCase()}`}
              <span className="f-sub">{def?.desc ?? g.desc}</span>
            </h2>
            {g.toggle && modeRow(g.toggle.label, g.toggle.key)}
            {items.map((f) => (
              <Fragment key={f.id}>
                <SettingRow
                  label={f.title}
                  hint={f.description}
                  preview={{
                    title: `# ${f.title.toLowerCase()}`,
                    body: `${f.description} ${f.versionTag}`,
                  }}
                >
                  <span className="s-add">
                    {f.schema.fields.length > 0 && (
                      <button
                        type="button"
                        className="btn"
                        aria-expanded={open === f.id}
                        aria-label={`Options for ${f.title}`}
                        onClick={() => setOpen(open === f.id ? null : f.id)}
                      >
                        options
                      </button>
                    )}
                    <OnOff
                      checked={f.enabled}
                      label={`Enable ${f.title}`}
                      onChange={(v) => void toggleFeature(f, v)}
                    />
                  </span>
                </SettingRow>
                {fail(f.id)}
                {open === f.id && (
                  <SchemaForm
                    fields={f.schema.fields}
                    options={f.options}
                    onSave={(o) => saveOptions(f.id, o)}
                  />
                )}
              </Fragment>
            ))}
          </section>
        );
      })}
    </section>
  );
}
