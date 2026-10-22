import "./keycaps.css";

interface Props {
  caps: string[];
}

/** One glass keycap per key of a chord. */
export function Keycaps({ caps }: Props) {
  return (
    <span className="keys">
      {caps.map((c) => (
        <kbd key={c} className="key">
          {c}
        </kbd>
      ))}
    </span>
  );
}
