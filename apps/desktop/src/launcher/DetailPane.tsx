import { shortcut } from "../shortcuts";
import type { Item } from "../types";
import { type DetailCtx, detailOf } from "./detail";

interface Props {
  item: Item;
  ctx: DetailCtx;
  onPress: (chord: string) => void;
}

/** Right pane of the selected row; each action sends the key its shortcut would. */
export function DetailPane({ item, ctx, onPress }: Props) {
  const detail = detailOf(item, ctx);
  return (
    <>
      <div className="rh">
        <h2 className="detail-title">{detail.title}</h2>
        <span className="tag">{detail.tag}</span>
      </div>
      {detail.facts.length > 0 && (
        <dl className="facts">
          {detail.facts.map((f) => (
            <div key={f.label} className="fact">
              <dt>{f.label}</dt>
              <dd>{f.value}</dd>
            </div>
          ))}
        </dl>
      )}
      <div className="detail-acts">
        {detail.actions.map((a) => (
          <button
            key={a.id}
            type="button"
            className={a.primary ? "detail-act pri" : "detail-act"}
            onClick={() => onPress(shortcut(a.id === "run" ? "open" : a.id).keys)}
          >
            {a.label}
            <span className="detail-keys">{a.keys}</span>
          </button>
        ))}
      </div>
    </>
  );
}
