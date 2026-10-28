// @vitest-environment jsdom
// Passcode lock, console log, custom shortcuts and connection export.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { press, query, renderApp, setupApp } from "@/test/app";

const h = setupApp();

describe("app safety and tools", () => {
  it("starts locked when a passcode is set and unlocks with it", async () => {
    h.backend.passcode = "2468";
    renderApp();
    const input = await screen.findByPlaceholderText("Passcode");
    fireEvent.change(input, { target: { value: "1111" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Unlock/ }));
    });
    expect(await screen.findByText("Wrong passcode.")).toBeTruthy();
    fireEvent.change(input, { target: { value: "2468" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Unlock/ }));
    });
    await waitFor(() => expect(screen.queryByPlaceholderText("Passcode")).toBeNull());
    expect(await screen.findByText("Local Postgres")).toBeTruthy();
  });

  it("ignores shortcuts while locked", async () => {
    h.backend.passcode = "2468";
    renderApp();
    await screen.findByPlaceholderText("Passcode");
    press("Slash", "/");
    expect(screen.queryByText("Keyboard shortcuts")).toBeNull();
  });

  it("shows the console log with ⇧⌘C and filters slow statements", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    press("KeyC", "C", { shiftKey: true });
    const panel = await screen.findByLabelText("Console log");
    expect(await within(panel).findByText("SELECT 1")).toBeTruthy();
    fireEvent.click(within(panel).getByText("Slow only"));
    await waitFor(() => expect(within(panel).queryByText("SELECT 1")).toBeNull());
    expect(within(panel).getByText("UPDATE t SET a = 1")).toBeTruthy();
  });

  it("changes a shortcut by pressing the new keys", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    press("Slash", "/");
    const dialog = await screen.findByRole("dialog");
    const row = within(dialog).getByText("Query history").closest(".shortcut-row") as HTMLElement;
    const keys = query("button.shortcut-keys", row);
    fireEvent.click(keys);
    expect(keys.textContent).toBe("Press keys…");
    fireEvent.keyDown(keys, { key: "j", code: "KeyJ", metaKey: true, ctrlKey: true, altKey: true });
    await waitFor(() => expect(h.backend.callsOf("save_settings").length).toBe(1));
    const saved = h.backend.lastCall("save_settings")?.args.settings.shortcuts;
    expect(saved.history).toMatch(/^Mod\+.*Alt\+J$/);
  });

  it("exports all connections without passwords", async () => {
    h.backend.savePath = "/tmp/libratable-export.json";
    renderApp();
    await screen.findByText("Local Postgres");
    fireEvent.contextMenu(document.querySelector(".home-list") as HTMLElement);
    fireEvent.click(await screen.findByText("Export all connections…"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByText("Continue"));
    await waitFor(() => expect(h.backend.callsOf("export_connections").length).toBe(1));
    expect(h.backend.lastCall("export_connections")?.args).toEqual({
      ids: [],
      path: "/tmp/libratable-export.json",
      passphrase: null,
    });
  });
});
