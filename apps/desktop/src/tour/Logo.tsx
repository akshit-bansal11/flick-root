interface Props {
  size?: number;
  /** Slides the bar in with a soft glow once; off under reduced motion via the motion tokens. */
  animated?: boolean;
}

/** The brand mark: a dark rounded square holding a cyan pill bar. Colors come from tokens. */
export function Logo({ size = 64, animated = false }: Props) {
  return (
    <svg
      className={animated ? "logo logo-in" : "logo"}
      width={size}
      height={size}
      viewBox="0 0 64 64"
      role="img"
      aria-label="Launcher logo"
    >
      <rect className="logo-tile" x="2" y="2" width="60" height="60" rx="14" />
      <rect className="logo-bar" x="12" y="27" width="40" height="10" rx="5" />
    </svg>
  );
}
