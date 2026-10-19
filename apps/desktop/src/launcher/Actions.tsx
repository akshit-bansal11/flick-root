export interface ActionKey {
  key: string;
  ctrl: boolean;
}

const ACTIONS: { label: string; hint: string; key: ActionKey }[] = [
  { label: "Open", hint: "↵", key: { key: "Enter", ctrl: false } },
  { label: "Open file location", hint: "Ctrl ↵", key: { key: "Enter", ctrl: true } },
  { label: "Copy path", hint: "Ctrl C", key: { key: "c", ctrl: true } },
];

interface Props {
  onPress: (key: string, ctrl: boolean) => void;
}

/** Find-mode action row; each button sends the same key the shortcut would. */
export function Actions({ onPress }: Props) {
  return (
    <div className="actions">
      {ACTIONS.map((a) => (
        <button
          key={a.label}
          type="button"
          className="action"
          onClick={() => onPress(a.key.key, a.key.ctrl)}
        >
          {a.label}
          <kbd className="kbd">{a.hint}</kbd>
        </button>
      ))}
    </div>
  );
}
