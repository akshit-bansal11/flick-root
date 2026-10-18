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

let root: Root | undefined;
let container: HTMLElement | undefined;
async function mount() {
  vi.mocked(api.listProviders).mockResolvedValue(list);
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
});
