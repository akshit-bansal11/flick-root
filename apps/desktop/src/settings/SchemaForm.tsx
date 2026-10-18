import { useState } from "react";
import { errorText, type Field } from "./types";

type Row = Record<string, string>;
type Draft = Record<string, unknown>;

interface Props {
  fields: Field[];
  options: Record<string, unknown>;
  onSave: (options: Record<string, unknown>) => Promise<void>;
}

const strings = (v: unknown): string[] => (Array.isArray(v) ? v.map(String) : []);
const isRow = (r: unknown): r is Row =>
  typeof r === "object" &&
  r !== null &&
  !Array.isArray(r) &&
  Object.values(r).every((x) => typeof x === "string");
const isRowArray = (v: unknown): v is Row[] => Array.isArray(v) && v.every(isRow);
const rows = (v: unknown): Row[] => (isRowArray(v) ? v : []);

// Rows have no id of their own: give each row object a stable React key that survives edits.
const ids = new WeakMap<Row, number>();
let nextId = 0;
const rowId = (r: Row): number => {
  const known = ids.get(r);
  if (known !== undefined) return known;
  ids.set(r, nextId);
  return nextId++;
};

/** Returns an error message for an out-of-range or non-numeric number field, else null. */
function numberError(f: Field, v: unknown): string | null {
  if (f.kind.type !== "number") return null;
  if (typeof v !== "number" || !Number.isFinite(v)) return `${f.label}: enter a number`;
  if (v < f.kind.min || v > f.kind.max)
    return `${f.label}: must be between ${f.kind.min} and ${f.kind.max}`;
  return null;
}

const withId = (old: Row, next: Row): Row => {
  ids.set(next, rowId(old));
  return next;
};

function move<T>(list: T[], from: number, to: number): T[] {
  const next = [...list];
  const [x] = next.splice(from, 1);
  next.splice(to, 0, x as T);
  return next;
}

/** Renders one form control per schema field and submits the whole options object. */
export function SchemaForm({ fields, options, onSave }: Props) {
  const [draft, setDraft] = useState<Draft>(() =>
    Object.fromEntries(
      fields
        .filter((f) => f.kind.type !== "secret")
        .map((f) => [f.key, options[f.key] ?? f.default]),
    ),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [newItem, setNewItem] = useState<Record<string, string>>({});
  const set = (k: string, v: unknown) => setDraft((d) => ({ ...d, [k]: v }));

  const submit = async () => {
    const bad = fields.map((f) => numberError(f, draft[f.key])).find(Boolean);
    if (bad) return setError(bad);
    setSaving(true);
    try {
      await onSave(draft);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
    setSaving(false);
  };

  const control = (f: Field) => {
    const v = draft[f.key];
    const k = f.kind;
    switch (k.type) {
      case "bool":
        return (
          <input
            type="checkbox"
            aria-label={f.label}
            checked={v === true}
            onChange={(e) => set(f.key, e.target.checked)}
          />
        );
      case "number":
        return (
          <input
            type="number"
            className="s-input"
            aria-label={f.label}
            min={k.min}
            max={k.max}
            step={k.step}
            value={typeof v === "number" && Number.isFinite(v) ? v : ""}
            onChange={(e) =>
              set(f.key, e.target.value === "" ? Number.NaN : Number(e.target.value))
            }
          />
        );
      case "text":
      case "path":
        return (
          <input
            className="s-input"
            aria-label={f.label}
            placeholder={k.type === "text" ? k.placeholder : undefined}
            value={typeof v === "string" ? v : ""}
            onChange={(e) => set(f.key, e.target.value)}
          />
        );
      case "select":
        return (
          <select
            className="s-select"
            aria-label={f.label}
            value={typeof v === "string" ? v : ""}
            onChange={(e) => set(f.key, e.target.value)}
          >
            {k.options.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </select>
        );
      case "secret":
        return <span className="s-help">Set in Providers; never stored in config.</span>;
      case "stringList": {
        const list = strings(v);
        return (
          <div className="s-roots">
            <ul className="s-list">
              {list.map((item, i) => (
                <li key={`${f.key}-${item}`} className="s-list-item">
                  <span className="s-path">{item}</span>
                  <button
                    type="button"
                    className="s-btn"
                    aria-label={`Remove ${item}`}
                    onClick={() =>
                      set(
                        f.key,
                        list.filter((_, j) => j !== i),
                      )
                    }
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
            <div className="s-add">
              <input
                className="s-input"
                aria-label={`New ${f.label}`}
                value={newItem[f.key] ?? ""}
                onChange={(e) => setNewItem((n) => ({ ...n, [f.key]: e.target.value }))}
              />
              <button
                type="button"
                className="s-btn"
                onClick={() => {
                  const t = (newItem[f.key] ?? "").trim();
                  if (!t || list.includes(t)) return;
                  set(f.key, [...list, t]);
                  setNewItem((n) => ({ ...n, [f.key]: "" }));
                }}
              >
                Add {f.label}
              </button>
            </div>
          </div>
        );
      }
      case "keyedList": {
        const list = rows(v);
        const edit = (i: number, col: string, val: string) =>
          set(
            f.key,
            list.map((r, j) => (j === i ? withId(r, { ...r, [col]: val }) : r)),
          );
        return (
          <div className="s-roots">
            <ul className="s-list">
              {list.map((r, i) => (
                <li key={rowId(r)} className="s-list-item">
                  {k.columns.map((c) => (
                    <input
                      key={c.key}
                      className="s-input"
                      aria-label={`${c.label} ${i + 1}`}
                      value={r[c.key] ?? ""}
                      onChange={(e) => edit(i, c.key, e.target.value)}
                    />
                  ))}
                  <button
                    type="button"
                    className="s-btn"
                    aria-label={`Move row ${i + 1} up`}
                    disabled={i === 0}
                    onClick={() => set(f.key, move(list, i, i - 1))}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="s-btn"
                    aria-label={`Move row ${i + 1} down`}
                    disabled={i === list.length - 1}
                    onClick={() => set(f.key, move(list, i, i + 1))}
                  >
                    ↓
                  </button>
                  <button
                    type="button"
                    className="s-btn"
                    aria-label={`Remove row ${i + 1}`}
                    onClick={() =>
                      set(
                        f.key,
                        list.filter((_, j) => j !== i),
                      )
                    }
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
            <button
              type="button"
              className="s-btn"
              onClick={() =>
                set(f.key, [...list, Object.fromEntries(k.columns.map((c) => [c.key, ""]))])
              }
            >
              Add row
            </button>
          </div>
        );
      }
    }
  };

  return (
    <form
      noValidate
      className="f-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      {fields.map((f) => (
        <div key={f.key} className="f-field">
          <div className="s-row-label">{f.label}</div>
          {f.help && <div className="s-help">{f.help}</div>}
          {f.restart && <div className="s-help">Applies after restart of this feature.</div>}
          {control(f)}
        </div>
      ))}
      {error && (
        <div className="s-error" role="alert">
          {error}
        </div>
      )}
      <button type="submit" className="s-btn" disabled={saving}>
        Save options
      </button>
    </form>
  );
}
