import { capsOf, groups, WHEN_TEXT } from "../shortcuts";

const TITLE_ID = "shortcut-sheet-title";

/** Every shortcut, grouped by scope. Covers the launcher body; Esc closes it. */
export function ShortcutSheet() {
  return (
    <section className="sheet" role="dialog" aria-modal="false" aria-labelledby={TITLE_ID}>
      <h2 id={TITLE_ID} className="sheet-title">
        Keyboard shortcuts
      </h2>
      {groups()
        .filter((g) => g.scope !== "tour")
        .map((g) => (
          <div key={g.scope} className="sheet-group">
            <h3 className="sheet-scope">{g.label}</h3>
            <dl className="sheet-list">
              {g.items.map((s) => (
                <div key={s.id} className="sheet-row">
                  <dt>
                    {s.label}
                    {s.when && <span className="sheet-when"> ({WHEN_TEXT[s.when]})</span>}
                  </dt>
                  <dd>
                    {capsOf(s.keys).map((c) => (
                      <kbd key={c} className="kbd">
                        {c}
                      </kbd>
                    ))}
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        ))}
    </section>
  );
}
