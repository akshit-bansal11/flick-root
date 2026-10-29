import { APP_DISPLAY_NAME } from "../brand";
import { keyWords, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { MODES } from "./reducer";

/** A shortcut as the panel shows it in words: `ctrl+,`. */
const chord = (id: string) => keyWords(shortcut(id).keys);

interface Props {
  /** Modes whose source is on, in hint order; a prefix that is off is not listed. */
  modes: Exclude<LaunchMode, "none">[];
}

/** The blank `all` tab with nothing to recall yet: what the bar does and how to start. */
export function Welcome({ modes }: Props) {
  return (
    <section className="welcome" aria-label="Welcome">
      <h1 className="welcome-title">{APP_DISPLAY_NAME}</h1>
      <p>Open apps, run commands, do math and ask your own AI from one bar.</p>
      <ul className="welcome-list">
        {modes.map((m) => (
          <li key={m}>
            <b>{MODES[m].prefix}</b> {MODES[m].label.toLowerCase()}
            <span className="welcome-desc">{MODES[m].desc}</span>
          </li>
        ))}
      </ul>
      <p className="welcome-keys">
        <em>{chord("openSettings")}</em> settings <em>{chord("sheet")}</em> shortcuts
      </p>
    </section>
  );
}
