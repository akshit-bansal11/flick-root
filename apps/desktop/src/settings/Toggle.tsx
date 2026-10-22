interface Props {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  describedBy?: string;
}

/** Pill switch. A native button, so Space and Enter toggle it. The one switch in Settings. */
export function Toggle({ checked, onChange, label, disabled, describedBy }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      aria-describedby={describedBy}
      disabled={disabled}
      className="s-switch"
      onClick={() => onChange(!checked)}
    />
  );
}
