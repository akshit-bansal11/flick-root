interface Props {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  describedBy?: string;
}

/** Two buttons, `on` and `off`, one pressed. Real buttons, so Tab, Space and Enter work. */
export function OnOff({ checked, onChange, label, disabled, describedBy }: Props) {
  const button = (on: boolean) => (
    <button
      type="button"
      aria-pressed={checked === on}
      disabled={disabled}
      onClick={() => {
        if (checked !== on) onChange(on);
      }}
    >
      {on ? "on" : "off"}
    </button>
  );
  return (
    <fieldset className="tg" aria-label={label} aria-describedby={describedBy}>
      {button(true)}
      {button(false)}
    </fieldset>
  );
}
