import { type ChangeEvent, useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { ThemeView } from "../theme";
import * as api from "./api";
import { errorText, type ImportPreview, MAX_THEME_BYTES, type MappingRow } from "./types";
import "./dialog.css";

interface Props {
  /** Gets the fresh theme list and the id of the theme just saved. */
  onImported: (themes: ThemeView[], id: string) => void;
}

const FOCUSABLE = "button:not(:disabled)";

/** "used as is", "raised to N:1" (muted and accent_text notes in `adjusted`) or "fallback". */
function resultOf(row: MappingRow, adjusted: string[]): string {
  if (!row.used) return "fallback";
  const note = adjusted.find((a) => a.startsWith(`${row.role} `));
  const to = note?.split("->")[1];
  return to ? `raised to ${to}:1` : "used as is";
}

/** The `import VS Code theme…` button: picks a file, previews it, opens the confirm dialog. */
export function ImportTheme({ onImported }: Props) {
  const input = useRef<HTMLInputElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [fileName, setFileName] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pick = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    setError(null);
    if (file.size > MAX_THEME_BYTES) {
      setError("That file is larger than 1 MB, which is too big for a theme.");
      return;
    }
    try {
      setPreview(await api.previewVscodeTheme(await file.text(), file.name));
      setFileName(file.name);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const close = () => {
    setPreview(null);
    button.current?.focus();
  };

  return (
    <>
      <input
        ref={input}
        type="file"
        accept=".json,.jsonc"
        hidden
        data-testid="import-file"
        onChange={pick}
      />
      <button type="button" ref={button} className="btn" onClick={() => input.current?.click()}>
        import VS Code theme…
      </button>
      {error && (
        <span role="alert" className="s-error">
          {error}
        </span>
      )}
      {preview &&
        createPortal(
          <PreviewDialog
            preview={preview}
            fileName={fileName}
            onCancel={close}
            onImported={onImported}
          />,
          document.body,
        )}
    </>
  );
}

interface DialogProps {
  preview: ImportPreview;
  fileName: string;
  onCancel: () => void;
  onImported: Props["onImported"];
}

function PreviewDialog({ preview, fileName, onCancel, onImported }: DialogProps) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
  }, []);

  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      onImported(await api.saveImportedTheme(preview.token), preview.id);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (e.key === "Tab" && ref.current) {
      const els = [...ref.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const first = els[0];
      const last = els[els.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  };

  return (
    <div className="key-dialog-scrim">
      <div
        ref={ref}
        className="key-dialog imp-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <h2 id={titleId}>Import {fileName}</h2>
        <table className="imp-map">
          <thead>
            <tr>
              <th scope="col">flick role</th>
              <th scope="col">VS Code key</th>
              <th scope="col">colour</th>
              <th scope="col">result</th>
            </tr>
          </thead>
          <tbody>
            {preview.rows.map((r) => (
              <tr key={r.role}>
                <th scope="row">{r.role}</th>
                <td>{r.key}</td>
                <td>
                  <span className="imp-sw" aria-hidden="true" style={{ backgroundColor: r.hex }} />
                  {r.hex}
                </td>
                <td>{resultOf(r, preview.adjusted)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p>
          Missing keys fall back to related colours; comments and trailing commas are allowed;
          colours with alpha are blended onto the panel.
        </p>
        {preview.notes.length > 0 && (
          <ul className="imp-notes">
            {preview.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        )}
        {error && (
          <p role="alert" className="key-dialog-error">
            {error}
          </p>
        )}
        <div className="key-dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            cancel
          </button>
          <button type="button" className="btn pri" disabled={busy} onClick={confirm}>
            {`import as "${preview.name}"`}
          </button>
        </div>
      </div>
    </div>
  );
}
