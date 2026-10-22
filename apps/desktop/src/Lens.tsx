import { type RefObject, useLayoutEffect, useRef } from "react";
import { measureRow } from "./useMotion";
import "./lens.css";

/** The selection lens: one glass pill moved by `useLens`. Decorative. */
export function Lens({ ref }: { ref: RefObject<HTMLDivElement | null> }) {
  return <div ref={ref} className="lens" aria-hidden="true" />;
}

const same = (a: readonly unknown[], b: readonly unknown[]) =>
  a.length === b.length && a.every((v, i) => Object.is(v, b[i]));

/**
 * Places `lens` over the option with id `selectedId` inside `list`. Measures only when the
 * selection or `deps` (the item list) changed, and writes transform (always) and height (when
 * it changed).
 * The first placement sets `data-lens` on the list and enables the glide on the next frame.
 */
export function useLens(
  list: RefObject<HTMLElement | null>,
  lens: RefObject<HTMLElement | null>,
  selectedId: string | undefined,
  deps: readonly unknown[],
) {
  const last = useRef<readonly unknown[] | null>(null);
  useLayoutEffect(() => {
    const next = [selectedId, ...deps];
    if (last.current && same(last.current, next)) return;
    last.current = next;
    const el = list.current;
    const layer = lens.current;
    if (!el || !layer) return;
    const row = selectedId ? el.querySelector<HTMLElement>(`#${selectedId}`) : null;
    const box = measureRow(row);
    if (!box) {
      delete el.dataset.lens;
      return;
    }
    const height = `${box.height}px`;
    if (layer.style.height !== height) layer.style.height = height;
    layer.style.transform = `translateY(${box.top}px)`;
    if (!el.dataset.lens) {
      el.dataset.lens = "1";
      requestAnimationFrame(() => {
        layer.dataset.glide = "1";
      });
    }
  });
}
