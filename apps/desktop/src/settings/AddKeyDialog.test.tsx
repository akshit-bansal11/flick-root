import { fireEvent, screen } from "@testing-library/react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { APP_DISPLAY_NAME } from "../brand";
import { AddKeyDialog } from "./AddKeyDialog";

let root: Root | undefined;
let container: HTMLElement | undefined;

function mount(over: Partial<Parameters<typeof AddKeyDialog>[0]> = {}) {
  const onSave = vi.fn(() => Promise.resolve());
  const onCancel = vi.fn();
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  act(() =>
    root?.render(
      <AddKeyDialog
        providerName="Acme"
        host="api.acme.test"
        keyless={false}
        mode="add"
        onSave={onSave}
        onCancel={onCancel}
        {...over}
      />,
    ),
  );
  return { onSave, onCancel };
}

afterEach(() => {
  act(() => root?.unmount());
  container?.remove();
});

const key = () => screen.getByLabelText("API key") as HTMLInputElement;
const save = () => screen.getByRole("button", { name: "save" }) as HTMLButtonElement;

describe("AddKeyDialog", () => {
  it("shows the exact warning and checkbox text", () => {
    mount();
    const text = `Your key is stored in Windows Credential Manager and is only sent to api.acme.test. No storage is completely safe: malware on your account, a compromised extension or a breach at the provider could still expose it. Add it at your own risk. Before saving, set a spending or usage limit in your provider's dashboard, and use a key created only for ${APP_DISPLAY_NAME} so you can revoke it on its own.`;
    expect(screen.getByText(text)).toBeTruthy();
    expect(screen.getByLabelText("I understand the risk and have set a usage limit.")).toBeTruthy();
  });

  it("renders nothing when keyless", () => {
    mount({ keyless: true });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("enables save only with checkbox and value", () => {
    mount();
    expect(save().disabled).toBe(true);
    fireEvent.change(key(), { target: { value: "sk-1" } });
    expect(save().disabled).toBe(true);
    fireEvent.click(screen.getByRole("checkbox"));
    expect(save().disabled).toBe(false);
  });

  it("passes the value to onSave then clears it", async () => {
    const { onSave } = mount();
    fireEvent.change(key(), { target: { value: "sk-1" } });
    fireEvent.click(screen.getByRole("checkbox"));
    await act(async () => {
      fireEvent.click(save());
    });
    expect(onSave).toHaveBeenCalledWith("sk-1", true);
    expect(key().value).toBe("");
  });

  it("shows the error when save rejects", async () => {
    mount({ onSave: () => Promise.reject(new Error("bad key")) });
    fireEvent.change(key(), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("checkbox"));
    await act(async () => {
      fireEvent.click(save());
    });
    expect(screen.getByRole("alert").textContent).toBe("bad key");
  });

  it("shows the message of a {code,message} rejection", async () => {
    mount({ onSave: () => Promise.reject({ code: "host_unbound", message: "host not bound" }) });
    fireEvent.change(key(), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("checkbox"));
    await act(async () => {
      fireEvent.click(save());
    });
    expect(screen.getByRole("alert").textContent).toBe("host not bound");
  });

  it("cancels on Escape", () => {
    const { onCancel } = mount();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalled();
  });
});
