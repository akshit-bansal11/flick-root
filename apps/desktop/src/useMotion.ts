import { useState } from "react";

/** Top and height of a row inside its offset parent, or null when it is not laid out. */
export function measureRow(el: { offsetTop: number; offsetHeight: number } | null | undefined) {
  if (!el || el.offsetHeight <= 0) return null;
  return { top: el.offsetTop, height: el.offsetHeight };
}

/**
 * Keeps the last non-null value around after `value` becomes null so an exit animation can play.
 * Call `onEnd` from onAnimationEnd of the leaving element to drop it.
 */
export function useExit<T>(value: T | null) {
  const [last, setLast] = useState<T | null>(value);
  if (value !== null && value !== last) setLast(value);
  return {
    shown: value ?? last,
    leaving: value === null && last !== null,
    onEnd: () => setLast(null),
  };
}
