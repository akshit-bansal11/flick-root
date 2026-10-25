import { useEffect, useRef } from "react";

/**
 * Two buttons that replace each other (ask / confirm). Call `arm()` right before flipping `on`;
 * focus then moves to the button that took the place of the one that unmounted.
 */
export function useSwapFocus(on: boolean) {
  const whenOn = useRef<HTMLButtonElement>(null);
  const whenOff = useRef<HTMLButtonElement>(null);
  const armed = useRef(false);
  useEffect(() => {
    if (!armed.current) return;
    armed.current = false;
    (on ? whenOn : whenOff).current?.focus();
  }, [on]);
  return {
    whenOn,
    whenOff,
    arm: () => {
      armed.current = true;
    },
  };
}
