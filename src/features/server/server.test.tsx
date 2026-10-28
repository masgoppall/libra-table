// @vitest-environment jsdom
// Server tools: process list (cancel / kill with confirmation), users (masked password
// preview), backup from "Open database" with progress events, restore, command line.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it } from "vitest";
import { dispatch } from "@/lib/actions";
import { openWorkspace, query, renderApp, setupApp } from "@/test/app";

const h = setupApp();

async function confirmDialog() {
  const dialogs = await screen.findAllByRole("dialog");
  return dialogs[dialogs.length - 1] as HTMLElement;
}

describe("server tools", () => {
  it("lists processes and cancels a query after showing the statement", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    act(() => void dispatch("process-list"));
    const row = (await screen.findByText("SELECT pg_sleep(600)")).closest("tr") as HTMLElement;
    expect(within(row).getByText("1m 05s")).toBeTruthy();
    fireEvent.click(within(row).getByText("Cancel"));
    const dialog = await confirmDialog();
    expect(within(dialog).getByText("SELECT pg_cancel_backend(101)")).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Cancel query" }));
    });
    await waitFor(() => expect(h.backend.callsOf("run_process_action").length).toBe(1));
    expect(h.backend.lastCall("run_process_action")?.args).toMatchObject({
      id: "c-local",
      processId: 101,
      action: "cancel",
      confirmed: true,
    });
  });

  it("asks to type the id before killing LibraTable's own connection", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    act(() => void dispatch("process-list"));
    const badge = await screen.findByText("LibraTable", { selector: ".own-badge" });
    const own = badge.closest("tr") as HTMLElement;
    fireEvent.click(within(own).getByText("Kill"));
    const dialog = await confirmDialog();
    expect(within(dialog).getByText(/LibraTable's own connection/)).toBeTruthy();
    const ok = within(dialog).getByRole("button", { name: "Kill connection" }) as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "102" } });
    expect(ok.disabled).toBe(false);
  });

  it("creates a role; the preview masks the password", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    act(() => void dispatch("users"));
    await screen.findByText("reader");
    fireEvent.click(screen.getByText("New role…"));
    const form = await confirmDialog();
    fireEvent.change(within(form).getByPlaceholderText("role_name"), {
      target: { value: "libratable_srv_app" },
    });
    const pw = form.querySelector("input[type=password]") as HTMLInputElement;
    fireEvent.change(pw, { target: { value: "hunter2" } });
    fireEvent.click(within(form).getByText("Create…"));
    const preview = await waitFor(async () => {
      const d = await confirmDialog();
      expect(within(d).queryByText(/CREATE ROLE/)).toBeTruthy();
      return d;
    });
    expect(preview.textContent).toContain("PASSWORD '********'");
    expect(preview.textContent).not.toContain("hunter2");
    await act(async () => {
      fireEvent.click(within(preview).getByRole("button", { name: "Create role" }));
    });
    await waitFor(() => expect(h.backend.callsOf("apply_user_change").length).toBe(1));
    const change = h.backend.lastCall("apply_user_change")?.args.change;
    expect(change).toMatchObject({ kind: "create_role", password: "hunter2" });
    expect(change.role).toMatchObject({ name: "libratable_srv_app", login: true });
  });

  it("backs up a database from the Open database sheet and follows the progress", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    act(() => void dispatch("switch-database"));
    fireEvent.contextMenu(await within(query(".dbs-sheet")).findByText("analytics"));
    fireEvent.click(await screen.findByText("Backup…"));
    const dialog = await confirmDialog();
    fireEvent.click(within(dialog).getAllByText("Choose…")[0] as HTMLElement);
    await screen.findByText(/--file=\/tmp\/app-backup.sql/);
    expect(within(dialog).getByText("pg_dump (PostgreSQL) 16.4")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Back up" }));
    await waitFor(() => expect(h.backend.callsOf("start_backup").length).toBe(1));
    const call = h.backend.lastCall("start_backup")?.args;
    expect(call).toMatchObject({ id: "c-local", database: "analytics" });
    expect(call?.request).toMatchObject({ path: "/tmp/app-backup.sql", format: "plain" });
    const base = { jobId: call?.jobId, total: null, error: null, path: "/tmp/app-backup.sql" };
    await act(() => emit("tool-progress", { ...base, line: "dumping table", bytes: 2048 }));
    expect(await screen.findByText("dumping table")).toBeTruthy();
    expect(screen.getByText(/2.0 KB written/)).toBeTruthy();
    await act(() => emit("tool-progress", { ...base, line: null, bytes: 4096, done: true }));
    await waitFor(() => expect(screen.queryByText("Backup · analytics")).toBeNull());
  });

  it("confirms a restore with its command, and shows the command line", async () => {
    h.backend.tools.pickedPath = "/tmp/app-backup.sql";
    renderApp();
    await openWorkspace("Local Postgres");
    act(() => void dispatch("restore"));
    const dialog = await confirmDialog();
    fireEvent.click(within(dialog).getAllByText("Choose…")[0] as HTMLElement);
    await screen.findByText(/psql --dbname=app < \/tmp\/app-backup.sql/);
    fireEvent.click(within(dialog).getByRole("button", { name: "Restore…" }));
    const confirm = await waitFor(async () => {
      const d = await confirmDialog();
      expect(d).not.toBe(dialog);
      return d;
    });
    await act(async () => {
      fireEvent.click(within(confirm).getByRole("button", { name: "Restore" }));
    });
    await waitFor(() => expect(h.backend.callsOf("start_restore").length).toBe(1));
    expect(h.backend.lastCall("start_restore")?.args).toMatchObject({ confirmed: true });

    act(() => void dispatch("command-line"));
    const cli = await screen.findByText("/usr/bin/psql -h 127.0.0.1 -p 5432 -U postgres -d app");
    expect(cli).toBeTruthy();
    expect(query(".command-line").textContent).not.toContain("password");
  });
});
