import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { Features } from "./Features";
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

const feature: FeatureView = {
  id: "demo",
  title: "Demo",
  versionTag: "v1",
  description: "A demo",
  enabled: false,
  status: "stopped",
  error: null,
  options: {},
  schema: {
    fields: [
      f("on", { type: "bool" }, false),
      f("days", { type: "number", min: 1, max: 30, step: 1 }, 7, true),
      f("name", { type: "text", placeholder: "x" }, "a"),
      f("pick", { type: "select", options: ["x", "y"] }, "x"),
      f("tags", { type: "stringList" }, ["t1"]),
      f("sites", { type: "keyedList", columns: [{ key: "k", label: "Key" }] }, [
        { k: "one" },
        { k: "two" },
      ]),
    ],
  },
};

let root: Root | undefined;

async function mount() {
  vi.mocked(api.getSettings).mockResolvedValue({ findEnabled: true, queryEnabled: false } as never);
  vi.mocked(api.listFeatures).mockResolvedValue({ features: [feature], onCount: 0 });
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Features />));
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

describe("Features", () => {
  it("Command toggle is locked on", async () => {
    await mount();
    const t = screen.getByRole("switch", { name: "Command" }) as HTMLButtonElement;
    expect(t.disabled).toBe(true);
    expect(t.getAttribute("aria-checked")).toBe("true");
  });

  it("toggling calls setFeatureEnabled and reverts on rejection", async () => {
    await mount();
    vi.mocked(api.setFeatureEnabled).mockRejectedValue({ code: "x", message: "nope" });
    const t = screen.getByRole("switch", { name: "Enable Demo" });
    await act(async () => {
      fireEvent.click(t);
    });
    expect(api.setFeatureEnabled).toHaveBeenCalledWith("demo", true);
    expect(t.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("alert").textContent).toBe("nope");
  });

  it("renders every kind, reorders keyed rows and submits the options object", async () => {
    await mount();
    vi.mocked(api.setFeatureOptions).mockResolvedValue(feature);
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Options for Demo" }));
    });
    act(() => {
      fireEvent.click(screen.getByLabelText("on"));
      fireEvent.change(screen.getByLabelText("days"), { target: { value: "9" } });
      fireEvent.change(screen.getByLabelText("pick"), { target: { value: "y" } });
      fireEvent.change(screen.getByLabelText("New tags"), { target: { value: "t2" } });
      fireEvent.click(screen.getByRole("button", { name: "Add tags" }));
      fireEvent.click(screen.getByRole("button", { name: "Move row 1 down" }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save options" }));
    });
    expect(api.setFeatureOptions).toHaveBeenCalledWith("demo", {
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
