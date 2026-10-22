import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Effect, Event } from "../launcher/reducer";
import { createDemoRunner, DEMO_ANSWER, demoSearch, ghostType, tourEventOf } from "./demo";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("ghostType", () => {
  it("types one character per tick and reports done", () => {
    const out: string[] = [];
    const done = vi.fn();
    ghostType("abc", (t) => out.push(t), done, 45);
    vi.advanceTimersByTime(44);
    expect(out).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(out).toEqual(["a"]);
    vi.advanceTimersByTime(90);
    expect(out).toEqual(["a", "ab", "abc"]);
    expect(done).toHaveBeenCalledOnce();
  });
  it("stops when cancelled", () => {
    const out: string[] = [];
    const cancel = ghostType("abc", (t) => out.push(t), undefined, 45);
    vi.advanceTimersByTime(45);
    cancel();
    vi.advanceTimersByTime(500);
    expect(out).toEqual(["a"]);
  });
});

describe("demoSearch", () => {
  it("filters apps, files and gives canned command results", () => {
    expect(demoSearch("command", ["apps"], "spot").map((i) => i.title)).toEqual(["Spotify"]);
    expect(demoSearch("command", ["apps"], "notepad").length).toBe(2);
    expect(demoSearch("find", ["files"], "report").length).toBeGreaterThan(0);
    const math = demoSearch("command", ["math"], "5 km to mi")[0];
    expect(math?.actions).toEqual([{ type: "copyResult" }]);
    expect(demoSearch("command", ["syscmd"], "lock")[0]?.id).toBe("cmd:lock");
    expect(demoSearch("query", [], "x")).toEqual([]);
  });
  it("lists apps, programs, files, folders and projects in order", () => {
    const ids = demoSearch("find", ["apps", "files"], "").map((i) => i.id);
    expect(ids.indexOf("prog:notepadpp")).toBeGreaterThan(ids.indexOf("app:notepad"));
    expect(ids.indexOf("file:reports")).toBeGreaterThan(ids.indexOf("file:budget"));
    expect(demoSearch("command", ["projects"], "").map((i) => i.title)).toEqual([
      "flick",
      "notes-app",
    ]);
  });
});

describe("demo runner", () => {
  const setup = () => {
    const dispatched: Event[] = [];
    const toasts: string[] = [];
    const runner = createDemoRunner({
      dispatch: (e) => dispatched.push(e),
      toast: (m) => toasts.push(m),
    });
    return { dispatched, toasts, runner };
  };

  it("answers a search with demo items", () => {
    const { dispatched, runner } = setup();
    runner.run({ type: "search", seq: 3, mode: "find", text: "report", sources: ["files"] });
    vi.advanceTimersByTime(1);
    expect(dispatched[0]).toMatchObject({ type: "results", seq: 3 });
  });
  it("turns system effects into toasts and never reaches IPC", () => {
    const { toasts, runner } = setup();
    runner.run({ type: "launch", itemId: "app:spotify", admin: false });
    runner.run({ type: "reveal", itemId: "file:report" });
    runner.run({ type: "hide" });
    runner.run({ type: "copyText", text: "x" });
    expect(toasts).toEqual([
      "Would open Spotify",
      "Would show it in Explorer",
      "In the real launcher Esc hides it",
    ]);
  });
  it("streams the canned answer and stops on cancel", () => {
    const { dispatched, runner } = setup();
    runner.run({ type: "ask", token: 1, text: "q", effort: "fast" });
    vi.advanceTimersByTime(2000);
    const deltas = dispatched.filter((e) => e.type === "ask" && e.event.type === "delta");
    expect(deltas).toHaveLength(DEMO_ANSWER.length);
    expect(dispatched.at(-1)).toMatchObject({ event: { type: "done" } });

    const second = setup();
    second.runner.run({ type: "ask", token: 2, text: "q", effort: "fast" });
    second.runner.run({ type: "cancelAsk", token: 2 });
    vi.advanceTimersByTime(2000);
    expect(second.dispatched).toEqual([]);
  });
  it("loads two thinking modes", () => {
    const { dispatched, runner } = setup();
    runner.run({ type: "loadThinkingModes" });
    vi.advanceTimersByTime(1);
    const first = dispatched[0];
    expect(first?.type).toBe("thinkingModes");
    expect(first && "modes" in first ? first.modes : []).toHaveLength(2);
  });
});

describe("tourEventOf", () => {
  it("maps effects to tour events", () => {
    const ev = (e: Effect) => tourEventOf(e);
    expect(ev({ type: "launch", itemId: "app:chrome", admin: false })).toBe("launchApp");
    expect(ev({ type: "launch", itemId: "file:report", admin: false })).toBe("openFile");
    expect(ev({ type: "launch", itemId: "cmd:lock", admin: false })).toBe("runCommand");
    expect(ev({ type: "launch", itemId: "projects:flick", admin: false })).toBe("openProject");
    expect(ev({ type: "reveal", itemId: "file:report" })).toBe("openFile");
    expect(ev({ type: "copyText", text: "1" })).toBe("copied");
    expect(ev({ type: "setThinkingMode", id: "x" })).toBe("modelSwitched");
    expect(ev({ type: "hide" })).toBeNull();
  });
});
