import type { Attachment as AttachmentData } from "../launcher/reducer";

interface Props {
  attachment: AttachmentData;
  onRemove: () => void;
}

/** The captured text, shown before anything is sent. Blocked text shows why instead of the text. */
export function Attachment({ attachment: a, onRemove }: Props) {
  return (
    <section className="attach" aria-label="Attached text">
      <div className="attach-head">
        <span>
          {a.sourceApp ? `From ${a.sourceApp} · ` : ""}
          {a.length} characters
        </span>
        <button type="button" className="attach-remove" tabIndex={-1} onClick={onRemove}>
          Remove
        </button>
      </div>
      {a.blocked ? (
        <p className="attach-blocked" role="alert">
          {a.reason ?? "This text cannot be sent."}
        </p>
      ) : (
        <pre className="attach-text">{a.preview}</pre>
      )}
    </section>
  );
}
