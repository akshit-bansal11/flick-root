import { useRef } from "react";
import { LauncherView } from "./LauncherView";
import { useLauncher } from "./useLauncher";

export function Launcher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const launcher = useLauncher(inputRef);
  return <LauncherView {...launcher} inputRef={inputRef} />;
}
