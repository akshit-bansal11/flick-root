import { capsText, shortcut } from "../shortcuts";

const ACTIONS = ["open", "location", "copyPath"] as const;

interface Props {
  onPress: (chord: string) => void;
}

/** Find-mode action row; each button sends the same key the shortcut would. */
export function Actions({ onPress }: Props) {
  return (
    <div className="actions">
      {ACTIONS.map((id) => (
        <button
          key={id}
          type="button"
          className="action"
          onClick={() => onPress(shortcut(id).keys)}
        >
          {shortcut(id).label}
          <kbd className="kbd">{capsText(id)}</kbd>
        </button>
      ))}
    </div>
  );
}
