import { fireEvent, screen, within } from "@testing-library/react";
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
    const parser = within(screen.getByRole("group", { name: "Enable Built-in parser" }));
    expect(parser.getByRole("button", { name: "on" }).getAttribute("aria-pressed")).toBe("true");
    expect(parser.getByRole("button", { name: "on" }).hasAttribute("disabled")).toBe(true);
    expect(parser.getByRole("button", { name: "off" }).hasAttribute("disabled")).toBe(true);
    for (const t of ["Laya", "Kev", "Your AI provider", "Jev", "Custom"]) {
      const g = within(screen.getByRole("group", { name: `Enable ${t}` }));
      expect(g.getByRole("button", { name: "on" }).getAttribute("aria-pressed")).toBe("false");
      expect(g.getByRole("button", { name: "off" }).getAttribute("aria-pressed")).toBe("true");
    }
  });

  it("toggling calls setEngine", async () => {
    await mount();
    await act(async () => {
      const group = screen.getByRole("group", { name: "Enable Laya" });
      fireEvent.click(within(group).getByRole("button", { name: "on" }));
    });
    expect(api.setEngine).toHaveBeenCalledWith("laya", { enabled: true });
  });

  it("Jev without a key shows Add key and saves under the engine name", async () => {
    await mount();
    expect(screen.getByText("no key")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add key for Jev" }));
    });
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "jv-test" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "save" }));
    });
    expect(api.setSecret).toHaveBeenCalledWith("engine.jev@api.typesafe.ai", "jv-test", true);
  });
});
