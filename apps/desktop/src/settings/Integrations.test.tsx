import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { Integrations } from "./Integrations";
import type { IntegrationsView } from "./types";

vi.mock("./api", () => ({
  getIntegrations: vi.fn(),
  setIntegration: vi.fn(),
  setSecret: vi.fn(() => Promise.resolve({})),
  deleteSecret: vi.fn(() => Promise.resolve()),
  connectSpotify: vi.fn(),
  disconnectSpotify: vi.fn(() => Promise.resolve()),
  testCaptureTarget: vi.fn(),
  onSpotifyState: vi.fn(() => Promise.resolve(() => {})),
}));

const view: IntegrationsView = {
  spotify: { client_id: "", redirect_uri: "http://127.0.0.1:43919/callback", connected: false },
  youtube_use_api: false,
  youtube_key_present: false,
  obsidian_vault: "",
  obsidian_daily_folder: "",
  obsidian_daily_format: "YYYY-MM-DD",
  markdown_file: "",
  notion_database_id: "",
  notion_token_present: false,
  webhooks: [{ name: "ci", enabled: false, host: "hooks.test", url_present: false }],
};

let root: Root | undefined;
let container: HTMLElement | undefined;
async function mount(v: IntegrationsView = view) {
  vi.mocked(api.getIntegrations).mockResolvedValue(v);
  vi.mocked(api.setIntegration).mockResolvedValue(view);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => root?.render(<Integrations />));
}
afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
});

describe("Integrations", () => {
  it("renders every section and the redirect URI", async () => {
    await mount();
    for (const t of ["Spotify", "YouTube", "Obsidian", "Markdown file", "Notion", "Webhooks"]) {
      expect(screen.getAllByText(t).length).toBeGreaterThan(0);
    }
    expect(screen.getByText("http://127.0.0.1:43919/callback")).toBeTruthy();
    const connect = screen.getByRole("button", { name: "Connect Spotify" }) as HTMLButtonElement;
    expect(connect.disabled).toBe(true);
  });

  it("saving the Obsidian vault calls setIntegration", async () => {
    await mount();
    fireEvent.change(screen.getByLabelText("Vault folder"), { target: { value: "C:/Vault" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save vault folder" }));
    });
    expect(api.setIntegration).toHaveBeenCalledWith("obsidian", { vault: "C:/Vault" });
  });

  it("Notion Add key opens the dialog and saves under the bound name", async () => {
    await mount();
    await act(async () => {
      fireEvent.click(screen.getAllByRole("button", { name: "Add key" })[1] as HTMLElement);
    });
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "ntn-test" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "save" }));
    });
    expect(api.setSecret).toHaveBeenCalledWith(
      "integration.notion@api.notion.com",
      "ntn-test",
      true,
    );
  });

  it("webhook: add by name and host, URL goes through setSecret, remove", async () => {
    await mount();
    fireEvent.change(screen.getByLabelText("Webhook name"), { target: { value: "deploy" } });
    fireEvent.change(screen.getByLabelText("Webhook host"), { target: { value: "x.test" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add webhook" }));
    });
    expect(api.setIntegration).toHaveBeenCalledWith("webhook", { name: "deploy", host: "x.test" });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add URL for ci" }));
    });
    fireEvent.change(screen.getByLabelText("API key"), {
      target: { value: "https://hooks.test/s" },
    });
    fireEvent.click(screen.getByRole("checkbox", { name: /understand/ }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "save" }));
    });
    expect(api.setSecret).toHaveBeenCalledWith(
      "integration.webhook.ci@hooks.test",
      "https://hooks.test/s",
      true,
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Remove webhook ci" }));
    });
    expect(api.setIntegration).toHaveBeenCalledWith("webhook", { name: "ci", remove: true });
  });

  it("connects Spotify once a Client ID is saved and shows the name", async () => {
    vi.mocked(api.connectSpotify).mockResolvedValue({ connected: true, name: "Ada" });
    await mount({ ...view, spotify: { ...view.spotify, client_id: "abc" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect Spotify" }));
    });
    expect(screen.getByText("Connected as Ada")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Disconnect Spotify" }));
    });
    expect(api.disconnectSpotify).toHaveBeenCalled();
  });

  it("explains a busy port", async () => {
    vi.mocked(api.connectSpotify).mockRejectedValue({ code: "port_busy", message: "x" });
    await mount({ ...view, spotify: { ...view.spotify, client_id: "abc" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect Spotify" }));
    });
    expect(
      screen.getByText("Port 43919 is in use — close the app using it and retry"),
    ).toBeTruthy();
  });

  it("Test shows the result for a configured target and the error for a failing one", async () => {
    vi.mocked(api.testCaptureTarget).mockResolvedValueOnce({ ok: true, message: "Writable" });
    await mount({ ...view, markdown_file: "C:/n/inbox.md" });
    expect(screen.queryByRole("button", { name: "Test Obsidian" })).toBeNull();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Test Markdown file" }));
    });
    expect(api.testCaptureTarget).toHaveBeenCalledWith("markdown");
    expect(screen.getByRole("status").textContent).toBe("Writable");
    vi.mocked(api.testCaptureTarget).mockRejectedValueOnce({
      code: "not_configured",
      message: "Add the webhook URL first",
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Test Markdown file" }));
    });
    expect(screen.getByRole("alert").textContent).toContain("Add the webhook URL first");
  });

  it("a webhook with a stored URL gets a Send test button", async () => {
    vi.mocked(api.testCaptureTarget).mockResolvedValue({ ok: true, message: "Accepted" });
    const hook = { name: "ci", enabled: true, host: "hooks.test", url_present: true };
    await mount({ ...view, webhooks: [hook] });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Send test ci" }));
    });
    expect(api.testCaptureTarget).toHaveBeenCalledWith("webhook.ci");
  });
});
