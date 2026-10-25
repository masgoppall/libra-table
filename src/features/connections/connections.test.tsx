// @vitest-environment jsdom
// Connections page: list, form, prompts while connecting.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp, setupApp } from "@/test/app";

const h = setupApp();

describe("connections", () => {
  it("lists connections and filters them by search (CON-1)", async () => {
    renderApp();
    expect(await screen.findByText("Local Postgres")).toBeTruthy();
    expect(screen.getByText("Prod Shop")).toBeTruthy();
    const search = screen.getByPlaceholderText("Search connections");
    fireEvent.change(search, { target: { value: "shop" } });
    await waitFor(() => expect(screen.queryByText("Local Postgres")).toBeNull());
    expect(screen.getByText("Prod Shop")).toBeTruthy();
  });

  it("creates a connection through the driver picker and form (CON-3..7)", async () => {
    renderApp();
    fireEvent.click(await screen.findByText("New connection"));
    fireEvent.click(await screen.findByText("Continue"));
    const name = await screen.findByPlaceholderText("My database");
    fireEvent.change(name, { target: { value: "Staging" } });
    fireEvent.change(screen.getByPlaceholderText("127.0.0.1"), {
      target: { value: "db.internal" },
    });
    const pw = document.querySelector('input[type="password"]') as HTMLInputElement;
    fireEvent.change(pw, { target: { value: "s3cret" } });
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    await waitFor(() => expect(h.backend.callsOf("save_connection").length).toBe(1));
    const input = h.backend.lastCall("save_connection")?.args.input;
    expect(input.connection.name).toBe("Staging");
    expect(input.connection.db.host).toBe("db.internal");
    expect(input.secrets.dbPassword).toEqual({ action: "set", value: "s3cret" });
    expect(input.secrets.sshPassword).toEqual({ action: "keep" });
  });

  it("asks for a password when the connection uses 'Ask every time'", async () => {
    h.backend.connectScript.push(() => {
      throw {
        kind: "password_required",
        layer: "database",
        message: "Enter the database password.",
        data: { which: "database" },
      };
    });
    renderApp();
    fireEvent.doubleClick(await screen.findByText("Local Postgres"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Password"), { target: { value: "typed-pw" } });
    fireEvent.click(within(dialog).getByText("OK"));
    await screen.findByText("people");
    expect(h.backend.callsOf("connect")[1]?.args.prompted.dbPassword).toBe("typed-pw");
  });

  it("shows the host key fingerprint and trusts it on confirm (SSH-4)", async () => {
    h.backend.connectScript.push(() => {
      throw {
        kind: "ssh_host_key_unknown",
        layer: "ssh",
        message: "Can't verify the SSH host [bastion]:2222.",
        data: {
          host: "bastion",
          port: 2222,
          algorithm: "ssh-ed25519",
          fingerprint: "SHA256:abc123",
        },
      };
    });
    renderApp();
    fireEvent.doubleClick(await screen.findByText("Local Postgres"));
    expect(await screen.findByText("SHA256:abc123")).toBeTruthy();
    fireEvent.click(screen.getByText("Trust & continue"));
    await screen.findByText("people");
    expect(h.backend.lastCall("trust_host_key")?.args).toEqual({ host: "bastion", port: 2222 });
  });
});
