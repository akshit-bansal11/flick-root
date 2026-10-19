import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { Decision } from "./Decision";
import type { EngineView } from "./types";

vi.mock("./api", () => ({
  listEngines: vi.fn(),
  probeDecisionEngines: vi.fn(() => Promise.resolve([])),
  setEngine: vi.fn(),
  setSecret: vi.fn(() => Promise.resolve({})),
  deleteSecret: vi.fn(() => Promise.resolve()),
}));

const mk = (id: string, title: string, o: Partial<EngineView> = {}): EngineView => ({
  id,
  title,
  enabled: false,
  base_url: "",
  key_host: null,
  needs_key: false,
  key_present: false,
  candidates: 8,
  threshold: 0.6,
  timeout_ms: null,
  order: 0,
  ...o,
});
const list: EngineView[] = [
  mk("laya", "Laya", { base_url: "http://localhost:8000", key_host: "localhost", order: 0 }),
  mk("kev", "Kev", { order: 1 }),
  mk("llm", "Your AI provider", { order: 2 }),
  mk("jev", "Jev", {
    base_url: "https://api.typesafe.ai",
    key_host: "api.typesafe.ai",
    needs_key: true,
    order: 3,
  }),
  mk("custom", "Custom", { order: 4 }),
];

let root: Root | undefined;
let container: HTMLElement | undefined;
async function mount() {
  vi.mocked(api.listEngines).mockResolvedValue(list);
  vi.mocked(api.setEngine).mockResolvedValue(list);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => root?.render(<Decision />));
}
afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
});

describe("Decision", () => {
  it("shows the parser locked on and every engine off", async () => {
    await mount();
    expect((screen.getByLabelText("Enable Built-in parser") as HTMLInputElement).checked).toBe(
      true,
    );
    for (const t of ["Laya", "Kev", "Your AI provider", "Jev", "Custom"]) {
      expect((screen.getByLabelText(`Enable ${t}`) as HTMLInputElement).checked).toBe(false);
    }
  });

  it("toggling calls setEngine", async () => {
    await mount();
    await act(async () => {
      fireEvent.click(screen.getByLabelText("Enable Laya"));
    });
    expect(api.setEngine).toHaveBeenCalledWith("laya", { enabled: true });
  });

  it("Jev without a key shows Add key and saves under the engine name", async () => {
    await mount();
    expect(screen.getByText("no key")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getAllByRole("button", { name: "Add key" })[1] as HTMLElement);
    });
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "jv-test" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save" }));
    });
    expect(api.setSecret).toHaveBeenCalledWith("engine.jev@api.typesafe.ai", "jv-test", true);
  });
});
