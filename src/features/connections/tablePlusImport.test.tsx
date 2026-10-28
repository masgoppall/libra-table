// @vitest-environment jsdom
// Migrating from TablePlus: its encrypted export, the password prompt and the report.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp, setupApp } from "@/test/app";

const h = setupApp();
const PATH = "/Users/me/tableplus_connection_dump.tableplusconnection";

function typePassword(value: string) {
  const dialog = screen.getByRole("dialog");
  const input = dialog.querySelector('input[type="password"]') as HTMLInputElement;
  fireEvent.change(input, { target: { value } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
}

describe("import from TablePlus", () => {
  it("asks for the export password again when it's wrong, then reports what was left", async () => {
    const handlers = h.backend["handlers"];
    handlers["plugin:dialog|open"] = () => PATH;
    handlers.inspect_connections_file = () => ({
      names: [],
      hasSecrets: true,
      source: "tableplus",
      locked: true,
    });
    let attempts = 0;
    handlers.import_connections = () => {
      if (attempts++ === 0) {
        throw { kind: "validation", message: "The password is wrong, or the file is damaged." };
      }
      return {
        added: 46,
        skipped: [
          { name: "Old Mongo", reason: "Couchbase isn't supported by LibraTable" },
          { name: "Local Postgres", reason: "already in LibraTable" },
        ],
      };
    };
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New connection" }));
    fireEvent.click(await screen.findByRole("button", { name: "Import Connection" }));
    fireEvent.click(await screen.findByText("From TablePlus…"));

    expect(await screen.findByText("Import from TablePlus")).toBeTruthy();
    typePassword("first try");
    expect(await screen.findByText("Wrong password")).toBeTruthy();
    typePassword("export-pw");

    const alert = await screen.findByRole("alertdialog");
    expect(within(alert).getByText("Imported 46 connections from TablePlus.")).toBeTruthy();
    expect(within(alert).getByText(/1 connection you already had/)).toBeTruthy();
    expect(
      within(alert).getByText("Old Mongo: Couchbase isn't supported by LibraTable"),
    ).toBeTruthy();
    expect(h.backend.callsOf("import_connections").map((c) => c.args)).toEqual([
      { path: PATH, passphrase: "first try" },
      { path: PATH, passphrase: "export-pw" },
    ]);
    // The list is reloaded with what came over.
    expect(h.backend.callsOf("list_connections").length).toBeGreaterThan(1);
  });

  it("cancelling the password prompt imports nothing", async () => {
    const handlers = h.backend["handlers"];
    handlers["plugin:dialog|open"] = () => PATH;
    handlers.inspect_connections_file = () => ({
      names: [],
      hasSecrets: true,
      source: "tableplus",
      locked: true,
    });
    renderApp();
    await screen.findByText("Local Postgres");
    fireEvent.contextMenu(document.querySelector(".home-list") as HTMLElement);
    fireEvent.click(await screen.findByText("Import from TablePlus…"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(h.backend.callsOf("import_connections").length).toBe(0);
  });
});
