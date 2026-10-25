import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { About } from "./About";
import * as api from "./api";
import type { About as AboutInfo } from "./types";

vi.mock("./api", () => ({
  getAbout: vi.fn(),
  openLink: vi.fn(() => Promise.resolve()),
}));

const info: AboutInfo = {
  version: "9.8.7",
  license: "Test-Licence-1.0",
  links: {
    repo: "https://example.com/repo",
    docs: "https://example.com/docs",
    changelog: "https://example.com/repo/blob/main/CHANGELOG.md",
    themes: "https://example.com/repo/blob/main/THIRD-PARTY-THEMES.md",
    releases: "https://example.com/repo/releases",
  },
  logDir: null,
};

let root: Root | undefined;
let container: HTMLElement | undefined;
async function mount() {
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => root?.render(<About />));
}
afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
  vi.clearAllMocks();
});

describe("About", () => {
  it("renders version and licence", async () => {
    vi.mocked(api.getAbout).mockResolvedValue(info);
    await mount();
    expect(screen.getByText("9.8.7")).toBeTruthy();
    expect(screen.getByText("Test-Licence-1.0")).toBeTruthy();
  });

  it("has an open button per link and theme credits opens links.themes", async () => {
    vi.mocked(api.getAbout).mockResolvedValue(info);
    await mount();
    for (const name of ["source code", "docs", "changelog", "theme credits"]) {
      expect(screen.getByRole("button", { name: `Open ${name}` })).toBeTruthy();
    }
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Open theme credits" }));
    });
    expect(api.openLink).toHaveBeenCalledWith(info.links.themes);
  });

  it("renders nothing while pending", async () => {
    vi.mocked(api.getAbout).mockReturnValue(new Promise(() => {}));
    await mount();
    expect(container?.innerHTML).toBe("");
  });

  it("renders nothing when getAbout rejects", async () => {
    vi.mocked(api.getAbout).mockRejectedValue(new Error("boom"));
    await mount();
    expect(container?.innerHTML).toBe("");
  });
});
