// @vitest-environment jsdom
// Import wizard (existing and new table, production), SQL files, multi-table export.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it } from "vitest";
import { openWorkspace, renderApp, setupApp, sidebar } from "@/test/app";

const h = setupApp();

async function chooseFileAndContinue(dialog: HTMLElement) {
  fireEvent.click(within(dialog).getByText("Choose file…"));
  await within(dialog).findByText("Ann");
  fireEvent.click(within(dialog).getByText("Next"));
}

async function sectionMenu(item: string) {
  fireEvent.contextMenu(within(sidebar()).getByText("Tables"));
  fireEvent.click(await screen.findByText(item));
  return screen.findByRole("dialog");
}

describe("import wizard", () => {
  it("imports a CSV into an existing table with columns matched by name", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Import…"));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Import into people")).toBeTruthy();
    await chooseFileAndContinue(dialog);
    const target = (name: string) =>
      within(dialog).getByLabelText(`Target of ${name}`) as HTMLSelectElement;
    await waitFor(() => expect(target("ID").value).toBe("id"));
    expect([target("name").value, target("Note").value]).toEqual(["name", "note"]);
    fireEvent.change(target("Note"), { target: { value: "" } });
    fireEvent.change(within(dialog).getByLabelText("On error"), { target: { value: "skip" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    expect(await within(dialog).findByText("2 rows imported.")).toBeTruthy();
    const req = h.backend.lastCall("start_import")?.args.request;
    expect(req).toMatchObject({
      schema: "public",
      table: "people",
      onError: "skip",
      transaction: "run",
      mapping: [
        { source: 0, target: "id" },
        { source: 1, target: "name" },
      ],
      source: { path: "/tmp/people.csv", format: "csv", csv: { delimiter: ",", header: true } },
    });
  });

  it("creates a new table with edited types, after one confirmation", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    const dialog = await sectionMenu("Import into new table…");
    fireEvent.change(within(dialog).getByLabelText("Delimiter"), { target: { value: ";" } });
    await chooseFileAndContinue(dialog);
    const name = within(dialog).getByLabelText("New table name") as HTMLInputElement;
    expect(name.value).toBe("people");
    fireEvent.change(name, { target: { value: "imported" } });
    fireEvent.change(within(dialog).getByLabelText("Column 2 type"), {
      target: { value: "varchar(80)" },
    });
    fireEvent.click(within(dialog).getByLabelText("Import column 3"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    const confirm = await screen.findByText("Create table & import");
    expect(screen.getByText("-- create_table imported")).toBeTruthy();
    await act(async () => fireEvent.click(confirm));
    await screen.findByText("2 rows imported.");
    const change = h.backend.lastCall("apply_schema_change")?.args.change;
    expect(change.columns.map((c: { name: string; dataType: string }) => c.dataType)).toEqual([
      "integer",
      "varchar(80)",
    ]);
    const start = h.backend.lastCall("start_import")?.args;
    expect(start?.confirmed).toBe(true);
    expect(start?.request.source.csv).toMatchObject({ delimiter: ";" });
    expect(h.backend.lastCall("preview_import")?.args.newColumns).toEqual([
      { name: "ID", dataType: "integer" },
      { name: "name", dataType: "varchar(80)" },
    ]);
  });

  it("asks before importing on a production connection", async () => {
    renderApp();
    await openWorkspace("Prod Shop");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Import…"));
    const dialog = await screen.findByRole("dialog");
    await chooseFileAndContinue(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    const ok = await screen.findByText("Run on production");
    await act(async () => fireEvent.click(ok));
    await screen.findByText("2 rows imported.");
    expect(h.backend.callsOf("start_import").map((c) => c.args.confirmed)).toEqual([false, true]);
  });
});

describe("SQL files and multi-table export", () => {
  it("runs a SQL file and shows the summary", async () => {
    h.backend.tools.pickedPath = "/tmp/dump.sql";
    renderApp();
    await openWorkspace("Local Postgres");
    const dialog = await sectionMenu("Run SQL file…");
    fireEvent.click(within(dialog).getByText("Choose file…"));
    await waitFor(() => expect(within(dialog).getByDisplayValue("/tmp/dump.sql")).toBeTruthy());
    fireEvent.click(within(dialog).getByText("Run file"));
    await waitFor(() => expect(h.backend.callsOf("start_sql_file").length).toBe(1));
    const { jobId, request } = h.backend.lastCall("start_sql_file")?.args ?? {};
    expect(request).toEqual({ path: "/tmp/dump.sql", stopOnError: true });
    const summary = {
      statements: 3,
      failed: 1,
      rowsAffected: 2,
      errors: [{ index: 2, line: 4, sql: "SELEC 1", message: "syntax error" }],
      skippedCommands: 0,
      cancelled: false,
      stopped: true,
      fatal: null,
    };
    await act(() => emit("sql-file-progress", { jobId, progress: {}, done: true, summary }));
    expect(await within(dialog).findByText(/Stopped at the first error/)).toBeTruthy();
    expect(within(dialog).getByText("syntax error")).toBeTruthy();
  });

  it("exports the chosen tables to a folder", async () => {
    h.backend.tools.pickedPath = "/tmp/out";
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Export tables…"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByText("Choose folder…"));
    fireEvent.click(within(dialog).getByLabelText("JSON"));
    const run = await within(dialog).findByText("Export 1 table");
    await waitFor(() => expect(run.closest("button")?.disabled).toBe(false));
    fireEvent.click(run);
    await waitFor(() => expect(h.backend.callsOf("start_export_tables").length).toBe(1));
    expect(h.backend.lastCall("start_export_tables")?.args.request).toMatchObject({
      schema: "public",
      tables: ["people"],
      format: "json",
      folder: "/tmp/out",
    });
  });
});
