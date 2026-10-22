import { useRef } from "react";
import type { ShownEvent } from "../ipc";
import { LauncherView } from "./LauncherView";
import { useLauncher } from "./useLauncher";

/** `initialShown` is a show that arrived while the launcher was unmounted; it is handled once on mount. */
export function Launcher({ initialShown }: { initialShown?: ShownEvent | null }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const launcher = useLauncher(inputRef, initialShown);
  return <LauncherView {...launcher} inputRef={inputRef} />;
}
