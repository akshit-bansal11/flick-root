import { EFFORT_AUTO, type ThinkingMode } from "../types";

/** The mode with `id`, else the first one (the ask path falls back the same way). */
export function selectedMode(
  modes: readonly ThinkingMode[],
  id: string | null | undefined,
): ThinkingMode | undefined {
  return modes.find((m) => m.id === id) ?? modes[0];
}

/** The stored level when the model supports it, else `auto`. */
export function effectiveEffort(efforts: readonly string[], stored: string): string {
  return efforts.includes(stored) ? stored : EFFORT_AUTO;
}

/** The next of `auto` and the model's levels, wrapping; null when the model has no levels. */
export function nextEffort(efforts: readonly string[], stored: string): string | null {
  if (efforts.length === 0) return null;
  const cycle = [EFFORT_AUTO, ...efforts];
  const at = cycle.indexOf(effectiveEffort(efforts, stored));
  return cycle[(at + 1) % cycle.length] ?? null;
}
