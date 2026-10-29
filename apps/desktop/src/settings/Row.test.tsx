import { screen } from "@testing-library/react";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { PreviewProvider, SettingRow } from "./Row";

let root: Root | undefined;

function Harness() {
  const [page, setPage] = useState("one");
  return (
    <PreviewProvider resetKey={page}>
      <button type="button" onClick={() => setPage("two")}>
        next page
      </button>
      <SettingRow
        label="hotkey"
        value="Alt+Space"
        preview={{ title: "# hotkey", body: "Opens it." }}
      >
        <button type="button">change</button>
      </SettingRow>
      <SettingRow
        label="startup"
        hint="runs at login"
        preview={{ title: "# startup", body: "At login." }}
      >
        <button type="button">toggle</button>
      </SettingRow>
    </PreviewProvider>
  );
}

function focusIn(name: string) {
  const el = screen.getByRole("button", { name });
  act(() => {
    el.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
  });
}

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const container = document.body.appendChild(document.createElement("div"));
  root = createRoot(container);
  act(() => root?.render(<Harness />));
});
afterEach(() => {
  act(() => root?.unmount());
  document.body.replaceChildren();
});

const preview = () => screen.getByRole("complementary", { name: "Preview" });

describe("SettingRow and PreviewProvider", () => {
  it("renders the row label, value and hint", () => {
    expect(screen.getByText("hotkey")).toBeTruthy();
    expect(screen.getByText("Alt+Space")).toBeTruthy();
    expect(screen.getByText("runs at login")).toBeTruthy();
  });

  it("shows the focused row in the preview column", () => {
    expect(preview().textContent).toContain("# preview");
    focusIn("change");
    expect(preview().textContent).toContain("# hotkey");
    expect(preview().textContent).toContain("Opens it.");
    focusIn("toggle");
    expect(preview().textContent).toContain("At login.");
    expect(preview().textContent).not.toContain("Opens it.");
  });

  it("follows the pointer entering a row", () => {
    const row = screen.getByText("startup").parentElement;
    act(() => {
      row?.dispatchEvent(new MouseEvent("mouseenter"));
    });
    expect(preview().textContent).toContain("At login.");
  });

  it("renders the help sentence under the row, visible without focus", () => {
    act(() =>
      root?.render(
        <PreviewProvider resetKey="x">
          <SettingRow label="a" hint="h" help="Always visible." preview={{ title: "#", body: "b" }}>
            <button type="button">go</button>
          </SettingRow>
          <SettingRow label="b" hint="h">
            <button type="button">go2</button>
          </SettingRow>
        </PreviewProvider>,
      ),
    );
    const help = screen.getByText("Always visible.");
    expect(help.tagName).toBe("P");
    expect(help.className).toContain("s-help");
    expect(document.querySelectorAll(".s-help")).toHaveLength(1);
  });

  it("resets when the page (resetKey) changes", () => {
    focusIn("change");
    expect(preview().textContent).toContain("Opens it.");
    act(() => screen.getByRole("button", { name: "next page" }).click());
    expect(preview().textContent).toContain("# preview");
    expect(preview().textContent).not.toContain("Opens it.");
  });
});
