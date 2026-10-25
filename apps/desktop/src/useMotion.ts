import { useState } from "react";

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
