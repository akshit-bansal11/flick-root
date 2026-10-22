import { Keycaps } from "../Keycaps";
import { caps, shortcut } from "../shortcuts";

const ACTIONS = ["open", "location", "copyPath"] as const;

interface Props {
  onPress: (chord: string) => void;
  /** The selected row is a file or folder; Copy path only applies then. */
  pathItem: boolean;
  /** The selected row has a location; Show location only applies then. */
  revealable: boolean;
}

/** Find-mode action row; each button sends the same key the shortcut would. */
export function Actions({ onPress, pathItem, revealable }: Props) {
  return (
    <div className="actions">
      {ACTIONS.filter(
        (id) => (id !== "copyPath" || pathItem) && (id !== "location" || revealable),
      ).map((id) => (
        <button
          key={id}
          type="button"
          className="action"
          onClick={() => onPress(shortcut(id).keys)}
        >
          {shortcut(id).label}
          <Keycaps caps={caps(id)} />
        </button>
      ))}
    </div>
  );
}
