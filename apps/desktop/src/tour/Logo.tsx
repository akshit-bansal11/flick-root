interface Props {
  size?: number;
  /** Slides the bar in with a soft glow once; off under reduced motion via the motion tokens. */
  animated?: boolean;
}

/** The brand mark (assets/logo/mark.svg): a dark tile, a light "f", a cyan bar and dot. Colors come from tokens. */
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
      <rect className="logo-tile" width="64" height="64" rx="15" />
      <path className="logo-glyph" d="M20.5 40.5H29.5V50A4.5 4.5 0 0 1 20.5 50Z" />
      <path
        className="logo-glyph"
        d="M20.5 25.5V24A13.5 13.5 0 0 1 34 10.5H35A4.5 4.5 0 0 1 35 19.5H34A4.5 4.5 0 0 0 29.5 24V25.5Z"
      />
      <path className="logo-bar" d="M16 28.5H46A4.5 4.5 0 0 1 46 37.5H16A4.5 4.5 0 0 1 16 28.5Z" />
      <path className="logo-bar" d="M44.5 15A4.5 4.5 0 1 1 53.5 15A4.5 4.5 0 1 1 44.5 15Z" />
    </svg>
  );
}
