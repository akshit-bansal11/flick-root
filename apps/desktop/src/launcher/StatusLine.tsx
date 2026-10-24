import { APP_DISPLAY_NAME } from "../brand";
import { useExit } from "../useMotion";
import type { Status } from "./status";

interface Props {
  status: Status;
  /** Shows the short "Copied" confirmation. */
  copied?: boolean;
  /** Shows "Copy failed" instead. */
  copyFailed?: boolean;
}

/** Bottom line: brand, counts, a few key hints and the copy toast. */
export function StatusLine({ status, copied = false, copyFailed = false }: Props) {
  const toast = useExit(copyFailed ? "Copy failed" : copied ? "Copied" : null);
  return (
    <div className="foot">
      <span className="brand">{APP_DISPLAY_NAME}</span>
      <span className="count">{status.text}</span>
      <span className="acts">
        {toast.shown && (
          <span
            className={toast.leaving ? "copied out" : "copied"}
            role={toast.leaving ? undefined : "status"}
            aria-hidden={toast.leaving || undefined}
            onAnimationEnd={toast.leaving ? toast.onEnd : undefined}
          >
            {toast.shown}
          </span>
        )}
        {status.hints.map((h) => (
          <span key={h.id} className="act">
            <span className="hint-keys">{h.keys}</span> {h.label}
          </span>
        ))}
      </span>
    </div>
  );
}
