import "./keycaps.css";

interface Props {
  caps: string[];
  /** Hide from assistive tech when the chord is announced elsewhere (aria-keyshortcuts). */
  decorative?: boolean;
}

/** One glass keycap per key of a chord. */
export function Keycaps({ caps, decorative }: Props) {
  return (
    <span className="keys" aria-hidden={decorative || undefined}>
      {caps.map((c) => (
        <kbd key={c} className="key">
          {c}
        </kbd>
      ))}
    </span>
  );
}
