import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { measureRow, useExit } from "./useMotion";

describe("measureRow", () => {
  it("returns top and height", () => {
    expect(measureRow({ offsetTop: 12, offsetHeight: 44 })).toEqual({ top: 12, height: 44 });
  });
  it("is null when missing or not laid out", () => {
    expect(measureRow(null)).toBeNull();
    expect(measureRow({ offsetTop: 0, offsetHeight: 0 })).toBeNull();
  });
});

describe("useExit", () => {
  it("holds the last value while leaving until onEnd", () => {
    const { result, rerender } = renderHook(({ v }) => useExit<string>(v), {
      initialProps: { v: null as string | null },
    });
    expect(result.current.shown).toBeNull();
    rerender({ v: "Apps" });
    expect(result.current).toMatchObject({ shown: "Apps", leaving: false });
    rerender({ v: null });
    expect(result.current).toMatchObject({ shown: "Apps", leaving: true });
    act(() => result.current.onEnd());
    expect(result.current).toMatchObject({ shown: null, leaving: false });
  });
});
