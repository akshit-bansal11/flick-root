import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

/** What the preview column shows: a title and a prose description of the selected setting. */
export interface Preview {
  title: string;
  body: string;
}

type SetPreview = (p: Preview | null) => void;

const PreviewContext = createContext<SetPreview>(() => {});

/** Lets a row or page choose what the preview column shows; a no-op outside the provider. */
export function usePreview(): SetPreview {
  return useContext(PreviewContext);
}

interface ProviderProps {
  children: ReactNode;
  /** The current page; a selection made on another page is dropped. */
  resetKey: string;
}

/** Holds the selected preview and renders the column after `children`. */
export function PreviewProvider({ children, resetKey }: ProviderProps) {
  const [chosen, choose] = useState<{ key: string; preview: Preview | null }>({
    key: resetKey,
    preview: null,
  });
  const preview = chosen.key === resetKey ? chosen.preview : null;
  const setPreview = useCallback(
    (p: Preview | null) => choose({ key: resetKey, preview: p }),
    [resetKey],
  );
  return (
    <PreviewContext.Provider value={setPreview}>
      {children}
      <aside className="s-prev" aria-label="Preview">
        <h3>{preview?.title ?? "# preview"}</h3>
        <p>{preview?.body ?? "Focus a setting to see what it does."}</p>
      </aside>
    </PreviewContext.Provider>
  );
}

interface RowProps {
  label: string;
  /** Current value in the accent colour, left of the control. */
  value?: string;
  /** Muted one-line comment, shown when there is no value. */
  hint?: string;
  /** Shown in the preview column while the row has focus or the pointer. */
  preview?: Preview;
  /** The control: `OnOff`, a `.btn`, a select. */
  children?: ReactNode;
}

/** One config-file row: marker, label, value or comment, control. Focus or hover selects it. */
export function SettingRow({ label, value, hint, preview, children }: RowProps) {
  const ref = useRef<HTMLDivElement>(null);
  const setPreview = usePreview();
  // Native listeners: the row is not itself interactive, its controls are.
  useEffect(() => {
    const el = ref.current;
    if (!el || !preview) return;
    const show = () => setPreview(preview);
    el.addEventListener("focusin", show);
    el.addEventListener("mouseenter", show);
    return () => {
      el.removeEventListener("focusin", show);
      el.removeEventListener("mouseenter", show);
    };
  }, [preview, setPreview]);
  return (
    <div ref={ref} className="sr">
      <span className="sr-mark" aria-hidden="true">
        ›
      </span>
      <span>{label}</span>
      {value ? <span className="sr-val">{value}</span> : <span className="sr-hint">{hint}</span>}
      {children}
    </div>
  );
}
