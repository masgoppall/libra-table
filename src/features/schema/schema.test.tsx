// @vitest-environment jsdom
// Schema changes: new database, new table, column change, drop with typed name.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  openWorkspace,
  press,
  query,
  queryAll,
  renderApp,
  sidebar,
  tabAt,
  setupApp,
} from "@/test/app";

const h = setupApp();

const applied = () => h.backend.callsOf("apply_schema_change");

const spec = (dataType: string) => ({
  name: "note",
  dataType,
  nullable: true,
  default: null,
  primaryKey: false,
  autoIncrement: false,
  unique: false,
  comment: null,
});

describe("schema changes", () => {
  it("creates a database and opens it (multi database)", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(within(sidebar()).getByTitle("New database"));
    const dialog = await screen.findByRole("dialog");
    const name = within(dialog).getByPlaceholderText("e.g. shop_dev");
    fireEvent.change(name, { target: { value: "reports" } });
    await waitFor(() =>
      expect(h.backend.callsOf("preview_schema_change").length).toBeGreaterThan(0),
    );
    fireEvent.click(within(dialog).getByText("Create database"));
    await waitFor(() => expect(applied().length).toBe(1));
    expect(applied()[0]?.args).toMatchObject({
      id: "c-local",
      database: null,
      change: { kind: "create_database", name: "reports" },
    });
    const listed = () =>
      h.backend.callsOf("list_objects").some((c) => c.args.database === "reports");
    await waitFor(() => expect(listed()).toBe(true));
    expect(query(".db-switch-name").textContent).toBe("reports");
  });

  it("creates a table with the designer (⌘⇧N)", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyN", "N", { shiftKey: true });
    const name = await screen.findByPlaceholderText("table_name");
    fireEvent.change(name, { target: { value: "orders" } });
    fireEvent.click(screen.getByText("Add column"));
    const names = queryAll<HTMLInputElement>(
      ".edit-table input.cell-input[placeholder='column_name']",
    );
    fireEvent.change(names[1] as HTMLInputElement, { target: { value: "total" } });
    fireEvent.click(screen.getByText("Create table…"));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("-- create_table orders")).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Create table" }));
    });
    await waitFor(() => expect(applied().length).toBe(1));
    const change = applied()[0]?.args.change;
    expect(change).toMatchObject({ kind: "create_table", schema: "public", name: "orders" });
    const cols = change.columns.map((c: Record<string, unknown>) => [
      c.name,
      c.dataType,
      c.primaryKey,
      c.autoIncrement,
    ]);
    expect(cols).toEqual([
      ["id", "bigint", true, true],
      ["total", "text", false, false],
    ]);
    // The tab becomes the new table's structure.
    await waitFor(() => expect(tabAt(0).textContent).toContain("orders"));
  });

  it("changes a column type in the structure tab and applies it with ⌘S", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Edit structure"));
    const typeInputs = await waitFor(() => {
      const xs = queryAll<HTMLInputElement>(".edit-table input[list^='lt-types']");
      expect(xs.length).toBe(3);
      return xs;
    });
    fireEvent.change(typeInputs[2] as HTMLInputElement, { target: { value: "varchar(500)" } });
    expect(await screen.findByText("1 change not applied yet")).toBeTruthy();
    press("KeyS", "s");
    const dialog = await screen.findByRole("dialog");
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Apply changes" }));
    });
    await waitFor(() => expect(applied().length).toBe(1));
    const change = applied()[0]?.args.change;
    expect(change.kind).toBe("alter_table");
    expect(change.ops).toEqual([
      { op: "modify_column", original: spec("text"), column: spec("varchar(500)") },
    ]);
  });

  it("drops a table only after typing its name", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Drop table…"));
    // Postgres asks about CASCADE first.
    fireEvent.click(within(await screen.findByRole("dialog")).getByText("Continue"));
    const dialog = await screen.findByRole("dialog");
    const ok = within(dialog).getByRole("button", { name: "Drop table" }) as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "people" } });
    expect(ok.disabled).toBe(false);
    await act(async () => {
      fireEvent.click(ok);
    });
    await waitFor(() => expect(applied().length).toBe(1));
    expect(applied()[0]?.args.change).toEqual({
      kind: "drop_table",
      schema: "public",
      name: "people",
      cascade: false,
    });
  });
});
