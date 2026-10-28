// @vitest-environment jsdom
// The structure tab as TablePlus has it: popup cells, Data | Structure | Index in the
// same tab, the foreign key editor, renaming from the Name field and deleting columns.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, press, query, queryAll, renderApp, setupApp, sidebar } from "@/test/app";

const h = setupApp();

const applied = () => h.backend.callsOf("apply_schema_change");

async function openStructure() {
  renderApp();
  await openWorkspace("Local Postgres");
  fireEvent.contextMenu(within(sidebar()).getByText("people"));
  fireEvent.click(await screen.findByText("Edit structure"));
  return waitFor(() => {
    const rows = queryAll<HTMLTableRowElement>(".st-grid tbody tr[data-col]");
    expect(rows.length).toBe(3);
    return rows;
  });
}

/** The n-th value cell of a row (after the handle and the number). */
const cell = (row: Element | undefined, n: number) =>
  (row as HTMLElement).querySelectorAll<HTMLElement>("td.st-cell")[n] as HTMLElement;

async function applyWithSave() {
  press("KeyS", "s");
  const dialog = await screen.findByRole("dialog");
  await act(async () => {
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply changes" }));
  });
  await waitFor(() => expect(applied().length).toBe(1));
  return applied()[0]?.args.change;
}

describe("structure tab", () => {
  it("shows TablePlus' columns with NULL / EMPTY and the bottom bar", async () => {
    await openStructure();
    const heads = queryAll(".st-grid thead th").map((th) => th.textContent);
    expect(heads).toEqual([
      "",
      "#",
      "column_name",
      "data_type",
      "is_nullable",
      "column_default",
      "extra",
      "foreign_key",
      "comment",
      "",
    ]);
    const rows = queryAll(".st-grid tbody tr[data-col]");
    expect(cell(rows[0], 4).textContent).toBe("auto_increment");
    expect(cell(rows[2], 3).textContent).toBe("NULL");
    expect(cell(rows[2], 3).classList.contains("dim")).toBe(true);
    expect(cell(rows[2], 6).textContent).toBe("EMPTY");
    const segments = within(query(".st-foot"))
      .getAllByRole("tab")
      .map((t) => t.textContent);
    expect(segments).toEqual(["Data", "Structure", "Index"]);
    expect(screen.getByRole("button", { name: "Triggers" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "DDL" })).toBeTruthy();
    expect(query<HTMLInputElement>(".st-name").value).toBe("people");
    expect(query(".st-primary").textContent).toBe("id");
  });

  it("picks a type and NULL from the cells' popup menus", async () => {
    const rows = await openStructure();
    fireEvent.click(within(cell(rows[2], 1)).getByRole("button", { name: "Choose" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByText("text").closest(".menu-item")?.className).toContain("selected");
    fireEvent.click(within(menu).getByText("varchar"));
    expect(cell(rows[2], 1).textContent).toBe("varchar(255)");

    fireEvent.click(within(cell(rows[2], 2)).getByRole("button", { name: "Choose" }));
    fireEvent.click(within(await screen.findByRole("menu")).getByText("NO"));
    expect(cell(rows[2], 2).textContent).toBe("NO");

    const change = await applyWithSave();
    expect(change.ops).toEqual([
      expect.objectContaining({
        op: "modify_column",
        column: expect.objectContaining({ dataType: "varchar(255)", nullable: false }),
      }),
    ]);
  });

  it("switches the same tab between Data and Structure", async () => {
    await openStructure();
    fireEvent.click(screen.getByRole("tab", { name: "Data" }));
    await waitFor(() => expect(document.querySelector(".structure-tab")).toBeNull());
    const toStructure = await screen.findByRole("tab", { name: "Structure" });
    expect(document.querySelectorAll(".tabbar .tab").length).toBe(1);
    fireEvent.click(toStructure);
    expect(await screen.findByText("column_name")).toBeTruthy();
  });

  it("adds an index from the Index view", async () => {
    await openStructure();
    fireEvent.click(screen.getByRole("tab", { name: "Index" }));
    expect(await screen.findByText("index_name")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add index" }));
    const editor = query<HTMLInputElement>(".st-grid input.st-input");
    fireEvent.change(editor, { target: { value: "people_note_idx" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    const added = queryAll(".st-grid tbody tr.added")[0];
    fireEvent.click(within(cell(added, 3)).getByRole("button", { name: "Choose" }));
    fireEvent.click(within(await screen.findByRole("menu")).getByText("note"));
    const change = await applyWithSave();
    expect(change.ops).toEqual([
      {
        op: "add_index",
        index: { name: "people_note_idx", columns: ["note"], unique: false, algorithm: "" },
      },
    ]);
  });

  it("creates a foreign key from the foreign_key cell", async () => {
    const rows = await openStructure();
    fireEvent.click(cell(rows[2], 5));
    const editor = await screen.findByRole("dialog", { name: "Foreign key" });
    expect(within(editor).getByText(/Create a foreign key on/)).toBeTruthy();
    fireEvent.change(within(editor).getByLabelText("Table"), { target: { value: "orders" } });
    const column = within(editor).getByLabelText("Column") as HTMLSelectElement;
    await waitFor(() => expect(column.options.length).toBeGreaterThan(1));
    fireEvent.change(column, { target: { value: "id" } });
    fireEvent.change(within(editor).getByLabelText("On delete"), {
      target: { value: "CASCADE" },
    });
    fireEvent.click(within(editor).getByRole("button", { name: "Save" }));
    expect(cell(rows[2], 5).textContent).toBe("orders(id)");
    const change = await applyWithSave();
    expect(change.ops).toEqual([
      {
        op: "add_foreign_key",
        foreignKey: expect.objectContaining({
          columns: ["note"],
          refTable: "orders",
          refColumns: ["id"],
          onDelete: "CASCADE",
        }),
      },
    ]);
  });

  it("renames the table from Name and deletes a column with ⌫", async () => {
    const rows = await openStructure();
    fireEvent.change(query(".st-name"), { target: { value: "persons" } });
    fireEvent.mouseDown(rows[1] as HTMLElement);
    fireEvent.keyDown(query(".st-scroll"), { key: "Backspace" });
    expect((rows[1] as HTMLElement).classList.contains("dropped")).toBe(true);
    const change = await applyWithSave();
    expect(change).toMatchObject({
      kind: "alter_table",
      name: "people",
      renameTo: "persons",
      ops: [{ op: "drop_column", name: "name" }],
    });
    // The tab follows the new name.
    await waitFor(() => expect(query(".tabbar .tab").textContent).toContain("persons"));
  });

  it("discards the changes from the toolbar", async () => {
    const rows = await openStructure();
    fireEvent.mouseDown(rows[2] as HTMLElement);
    fireEvent.keyDown(query(".st-scroll"), { key: "Delete" });
    const discard = screen.getByRole("button", { name: "Discard changes" }) as HTMLButtonElement;
    expect(discard.disabled).toBe(false);
    fireEvent.click(discard);
    await waitFor(() => expect(queryAll(".st-grid tr.dropped").length).toBe(0));
  });
});
