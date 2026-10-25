export interface TermOption {
  value: string;
  label: string;
  hint?: string;
}

interface Props {
  legend: string;
  name: string;
  value: string;
  options: TermOption[];
  onChange: (value: string) => void;
}

/** Terminal radio group: native radios inside a fieldset, drawn as (•) / ( ) by firstrun.css. */
export function TermRadio({ legend, name, value, options, onChange }: Props) {
  return (
    <fieldset className="tr">
      <legend className="tr-legend">{legend}</legend>
      {options.map((o) => (
        <label key={o.value} className="tr-opt">
          <input
            type="radio"
            name={name}
            value={o.value}
            checked={value === o.value}
            onChange={() => onChange(o.value)}
          />
          <span className="tr-mark" aria-hidden="true" />
          <span>{o.label}</span>
          {o.hint && <span className="tr-hint">{o.hint}</span>}
        </label>
      ))}
    </fieldset>
  );
}
