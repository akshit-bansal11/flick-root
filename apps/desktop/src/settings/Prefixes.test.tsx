import { fireEvent, screen, within } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { Prefixes } from "./Prefixes";
import { PREFIX_GROUPS } from "./prefixGroups";
import type { FeatureView, Field } from "./types";

vi.mock("./api", () => ({
  getSettings: vi.fn(),
  listFeatures: vi.fn(),
  setFeatureEnabled: vi.fn(),
  setFeatureOptions: vi.fn(),
  setModeEnabled: vi.fn(),
  onFeaturesChanged: vi.fn(() => Promise.resolve(() => {})),
}));

const f = (key: string, kind: Field["kind"], def: unknown, restart = false): Field => ({
  key,
  label: key,
  help: "",
  kind,
  default: def,
  restart,
});

const make = (id: string, title: string, fields: Field[] = []): FeatureView => ({
  id,
  title,
  versionTag: "v1",
  description: `${title} description`,
  enabled: false,
  status: "stopped",
  error: null,
  options: {},
  schema: { fields },
});

const feature = make("math", "Demo", [
  f("on", { type: "bool" }, false),
  f("days", { type: "number", min: 1, max: 30, step: 1 }, 7, true),
  f("name", { type: "text", placeholder: "x" }, "a"),
  f("pick", { type: "select", options: ["x", "y"] }, "x"),
  f("tags", { type: "stringList" }, ["t1"]),
  f("sites", { type: "keyedList", columns: [{ key: "k", label: "Key" }] }, [
    { k: "one" },
    { k: "two" },
  ]),
]);

let root: Root | undefined;

const onOff = (label: string, state: "on" | "off") =>
  within(screen.getByRole("group", { name: label })).getByRole("button", { name: state });

// The heading's first child is its `# name` text; the description span follows it.
const headings = () =>
  screen.getAllByRole("heading", { level: 2 }).map((h) => h.firstChild?.textContent);

async function mount(features: FeatureView[] = [feature]) {
  vi.mocked(api.getSettings).mockResolvedValue({ findEnabled: true, queryEnabled: false } as never);
  vi.mocked(api.listFeatures).mockResolvedValue({ features, onCount: 0 });
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Prefixes />));
  await act(async () => {});
}

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
  vi.clearAllMocks();
});

describe("Prefixes", () => {
  it("renders groups in prefix order, each feature once", async () => {
    const all = PREFIX_GROUPS.flatMap((g) => g.features).map((id) => make(id, `Mod ${id}`));
    await mount(all);
    expect(headings()).toEqual([
      "# open",
      "# commands",
      "# calc",
      "# ask",
      "# projects",
      "# everywhere",
    ]);
    for (const m of all) expect(screen.getAllByText(m.title)).toHaveLength(1);
    expect(headings()).not.toContain("# other");
  });

  it("puts an unknown feature in Other", async () => {
    await mount([feature, make("brand-new", "Brand new")]);
    expect(headings().at(-1)).toBe("# other");
    expect(screen.getByRole("group", { name: "Enable Brand new" })).toBeTruthy();
  });

  it("has no locked Command row", async () => {
    await mount();
    expect(screen.queryByRole("group", { name: "Command" })).toBeNull();
  });

  it("mode toggles call setModeEnabled and roll back on rejection", async () => {
    await mount();
    vi.mocked(api.setModeEnabled).mockRejectedValue({ code: "x", message: "mode failed" });
    const t = onOff("Ask", "on");
    await act(async () => {
      fireEvent.click(t);
    });
    expect(api.setModeEnabled).toHaveBeenCalledWith("query", true);
    expect(t.getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("alert").textContent).toBe("mode failed");
  });

  it("toggling calls setFeatureEnabled and reverts on rejection", async () => {
    await mount();
    vi.mocked(api.setFeatureEnabled).mockRejectedValue({ code: "x", message: "nope" });
    const t = onOff("Enable Demo", "on");
    await act(async () => {
      fireEvent.click(t);
    });
    expect(api.setFeatureEnabled).toHaveBeenCalledWith("math", true);
    expect(t.getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("alert").textContent).toBe("nope");
  });

  it("renders every kind, reorders keyed rows and submits the options object", async () => {
    await mount();
    vi.mocked(api.setFeatureOptions).mockResolvedValue(feature);
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Options for Demo" }));
    });
    act(() => {
      fireEvent.click(screen.getByRole("checkbox", { name: "on" }));
      fireEvent.change(screen.getByLabelText("days"), { target: { value: "9" } });
      fireEvent.change(screen.getByLabelText("pick"), { target: { value: "y" } });
      fireEvent.change(screen.getByLabelText("New tags"), { target: { value: "t2" } });
      fireEvent.click(screen.getByRole("button", { name: "Add tags" }));
      fireEvent.click(screen.getByRole("button", { name: "Move row 1 down" }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save options" }));
    });
    expect(api.setFeatureOptions).toHaveBeenCalledWith("math", {
      on: true,
      days: 9,
      name: "a",
      pick: "y",
      tags: ["t1", "t2"],
      sites: [{ k: "two" }, { k: "one" }],
    });
  });

  it("rejects an out-of-range number without calling Rust", async () => {
    await mount();
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Options for Demo" }));
    });
    act(() => {
      fireEvent.change(screen.getByLabelText("days"), { target: { value: "99" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save options" }));
    });
    expect(api.setFeatureOptions).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toContain("between 1 and 30");
  });
});
