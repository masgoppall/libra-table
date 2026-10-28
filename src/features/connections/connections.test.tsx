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
    const search = screen.getByPlaceholderText("Search for connection...");
    fireEvent.change(search, { target: { value: "shop" } });
    await waitFor(() => expect(screen.queryByText("Local Postgres")).toBeNull());
    expect(screen.getByText("Prod Shop")).toBeTruthy();
  });

  it("creates a connection through the driver picker and form (CON-3..7)", async () => {
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New connection" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
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

  it("marks a missing name in red and explains it in a popup instead of saving", async () => {
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New connection" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    const name = await screen.findByPlaceholderText("My database");
    expect(name.getAttribute("aria-required")).toBe("true");
    expect(name.getAttribute("aria-invalid")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    const alert = await screen.findByRole("alertdialog");
    expect(within(alert).getByText("Give the connection a name.")).toBeTruthy();
    expect(name.getAttribute("aria-invalid")).toBe("true");
    expect(h.backend.callsOf("save_connection").length).toBe(0);
    fireEvent.click(within(alert).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    fireEvent.change(name, { target: { value: "Fixed" } });
    expect(name.getAttribute("aria-invalid")).toBeNull();
  });

  it("shows an error from the server in a popup", async () => {
    h.backend["handlers"].save_connection = () => {
      throw { kind: "validation", layer: "app", message: "Port is out of range." };
    };
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New connection" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    fireEvent.change(await screen.findByPlaceholderText("My database"), {
      target: { value: "Broken" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    const alert = await screen.findByRole("alertdialog");
    expect(within(alert).getByText("Couldn't save the connection")).toBeTruthy();
    expect(within(alert).getByText("Port is out of range.")).toBeTruthy();
  });

  it("selects a group on click and opens or closes it on double-click", async () => {
    h.backend.groups = [{ id: "g1", name: "Clients", parentId: null, collapsed: false }];
    renderApp();
    const row = (await screen.findByText("Clients")).closest(".tree-row") as HTMLElement;
    fireEvent.click(row);
    expect(row.className).toContain("selected");
    expect(h.backend.callsOf("save_group").length).toBe(0);
    fireEvent.doubleClick(row);
    await waitFor(() => expect(h.backend.lastCall("save_group")?.args.group.collapsed).toBe(true));
  });

  it("creates a group in the group dialog, with the plug icon until one is chosen", async () => {
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "New group" }));
    const dialog = await screen.findByRole("dialog", { name: "New group" });
    const save = within(dialog).getByRole("button", { name: "Save" }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    expect((within(dialog).getByText("Clear icon") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(within(dialog).getByPlaceholderText("Production servers..."), {
      target: { value: "Clients" },
    });
    fireEvent.click(save);
    await waitFor(() =>
      expect(h.backend.lastCall("save_group")?.args.group).toMatchObject({
        name: "Clients",
        parentId: null,
        icon: null,
      }),
    );
    expect(await screen.findByText("Clients")).toBeTruthy();
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

describe("welcome window", () => {
  it("backs up a database chosen from the welcome window", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    fireEvent.click(screen.getByText("Backup Database"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByText("Local Postgres"));
    fireEvent.click(within(dialog).getByText("Continue"));
    expect(await screen.findByText("Backup · app")).toBeTruthy();
    expect(h.backend.callsOf("connect")[0]?.args.id).toBe("c-local");
  });
});
