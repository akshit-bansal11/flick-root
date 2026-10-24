import "./keycaps.css";
import { keyWords } from "./shortcuts";

interface Props {
  caps: string[];
  /** Hide from assistive tech when the chord is announced elsewhere (aria-keyshortcuts). */
  decorative?: boolean;
}

/** A chord written as words, `ctrl+shift+enter`. */
export function Keycaps({ caps, decorative }: Props) {
  return (
    <kbd className="key" aria-hidden={decorative || undefined}>
      {keyWords(caps.join("+"))}
    </kbd>
  );
}
