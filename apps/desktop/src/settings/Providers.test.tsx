import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { Providers } from "./Providers";
import type { ProviderView } from "./types";

vi.mock("./api", () => ({
  listProviders: vi.fn(),
  probeProviders: vi.fn(),
  setProvider: vi.fn(),
  setProviderOrder: vi.fn(),
  setSecret: vi.fn(() => Promise.resolve({})),
  deleteSecret: vi.fn(() => Promise.resolve()),
  testSecret: vi.fn(),
}));

const base = {
  enabled: false,
  model: "",
  models: [] as string[],
  supports_effort: false,
  status: "off",
} as const;
const list: ProviderView[] = [
  {
    ...base,
    id: "ollama",
    title: "Ollama",
    group: "local",
    base_url: "http://localhost:11434",
    key_required: false,
    key: null,
  },
  {
    ...base,
    id: "openai",
    title: "OpenAI",
    group: "own_key",
    base_url: "https://api.openai.com/v1",
    key_required: true,
    key: null,
  },
  {
    ...base,
    id: "anthropic",
    title: "Anthropic",
    group: "own_key",
    base_url: "https://api.anthropic.com",
    key_required: true,
    key: {
      name: "provider.anthropic@api.anthropic.com",
      present: true,
      last4: "wxyz",
      host: "api.anthropic.com",
      added_at: 1,
      last_used: null,
    },
  },
];

const ported = {
  ...list[1],
  id: "ported",
  base_url: "https://api.acme.test:8443/v1",
} as ProviderView;
const second = { ...list[1], id: "second", title: "Second" } as ProviderView;

let root: Root | undefined;
let container: HTMLElement | undefined;
async function mount(l: ProviderView[] = list) {
  vi.mocked(api.listProviders).mockResolvedValue(l);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => root?.render(<Providers />));
}
afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
});
const click = (name: string | RegExp, i = 0) =>
  act(async () => {
    fireEvent.click(screen.getAllByRole("button", { name })[i] as HTMLElement);
  });

describe("Providers", () => {
  it("Add key saves with acknowledged=true; keyless has no Add key", async () => {
    await mount();
    expect(screen.getAllByRole("button", { name: "Add key" })).toHaveLength(1);
    await click("Add key");
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "sk-test" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await click("Save");
    expect(api.setSecret).toHaveBeenCalledWith("provider.openai@api.openai.com", "sk-test", true);
  });

  it("masks the key and Delete needs two steps", async () => {
    await mount();
    expect(screen.getByText(/•••• wxyz · api\.anthropic\.com/)).toBeTruthy();
    await click("Delete");
    expect(api.deleteSecret).not.toHaveBeenCalled();
    await click("Confirm delete");
    expect(api.deleteSecret).toHaveBeenCalledWith("provider.anthropic@api.anthropic.com");
  });

  it("warns when the base URL host changes on a keyed provider", async () => {
    await mount();
    await click("Options", 2);
    fireEvent.change(screen.getByLabelText("Base URL"), {
      target: { value: "https://other.example/v1" },
    });
    expect(screen.getByRole("alert").textContent).toMatch(/deletes the stored key/);
  });

  it("binds the secret to the hostname without the port", async () => {
    await mount([ported]);
    await click("Add key");
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "k" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await click("Save");
    expect(api.setSecret).toHaveBeenLastCalledWith("provider.ported@api.acme.test", "k", true);
  });

  it("does not warn on a port-only change", async () => {
    const keyed = { ...list[2], base_url: "https://api.anthropic.com:443/x" } as ProviderView;
    await mount([keyed]);
    await click("Options");
    fireEvent.change(screen.getByLabelText("Base URL"), {
      target: { value: "https://api.anthropic.com:8443/x" },
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("saves extra models as a trimmed list", async () => {
    await mount([list[0] as ProviderView]);
    await click("Options");
    fireEvent.change(screen.getByLabelText("Extra models"), {
      target: { value: " llama3 , ,qwen " },
    });
    await click("Save options");
    expect(api.setProvider).toHaveBeenCalledWith(
      "ollama",
      expect.objectContaining({ models: ["llama3", "qwen"] }),
    );
  });

  it("keeps reordering within a group", async () => {
    await mount([list[1] as ProviderView, list[0] as ProviderView, second]);
    await click("Options", 1);
    expect(
      (screen.getByRole("button", { name: "Move OpenAI up" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    await click("Move OpenAI down");
    expect(api.setProviderOrder).toHaveBeenCalledWith(["ollama", "second", "openai"]);
  });
});
