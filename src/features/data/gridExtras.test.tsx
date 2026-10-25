// @vitest-environment jsdom
// Data grid extras: hidden columns, foreign keys, Quick Look, enum editor.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  openWorkspace,
  press,
  query,
  queryAll,
  renderApp,
  setupApp,
  sidebar,
  tabs,
} from "@/test/app";

const h = setupApp();

/** The table item itself (other sidebar items may mention the table, e.g. triggers). */
function tableItem(name: string): HTMLElement {
  const items = queryAll(".side-item", sidebar());
  const item = items.find(
    (el) =>
      el.querySelector(".label")?.textContent === name && el.querySelector(".side-icon.table"),
  );
  if (!item) throw new Error(`No table  in the sidebar`);
  return item;
}

async function openTable(name: string) {
  renderApp();
  await openWorkspace("Local Postgres");
  fireEvent.click(tableItem(name));
}

const header = (name: string) =>
  queryAll(".grid-th").find((th) => th.querySelector(".th-name")?.textContent === name);

describe("grid extras", () => {
  it("hides a column from its header menu and shows it again from Columns", async () => {
    await openTable("people");
    await screen.findByText("Alice");
    fireEvent.contextMenu(header("note") as HTMLElement);
    fireEvent.click(await screen.findByText("Hide column"));
    await waitFor(() => expect(header("note")).toBeUndefined());
    const saved = h.backend.lastCall("save_table_state")?.args.tableState;
    expect(saved.hiddenColumns).toEqual(["note"]);

    fireEvent.click(screen.getByRole("button", { name: /Columns/ }));
    fireEvent.click(await screen.findByText("Show all columns"));
    await waitFor(() => expect(header("note")).toBeTruthy());
  });

  it("follows a foreign key to the referenced row", async () => {
    await openTable("orders");
    await screen.findByText("new");
    // The FK column is marked in its header.
    await waitFor(() => expect(header("person_id")?.querySelector(".th-fk")).toBeTruthy());
    const row = queryAll(".grid-row")[0] as HTMLElement;
    fireEvent.contextMenu(queryAll(".grid-cell", row)[2] as HTMLElement);
    fireEvent.click(await screen.findByText("Open referenced row in people"));
    await waitFor(() => expect(tabs().map((t) => t.textContent)).toContain("people"));
    await waitFor(() => {
      const req = h.backend.lastCall("fetch_page")?.args.request;
      expect(req.table).toBe("people");
      expect(req.filters).toEqual([
        { enabled: true, column: "id", operator: "eq", value: "2", value2: "" },
      ]);
    });
  });

  it("shows a JSON cell as a tree in Quick Look (Space)", async () => {
    await openTable("orders");
    await screen.findByText("new");
    const row = queryAll(".grid-row")[0] as HTMLElement;
    fireEvent.mouseDown(queryAll(".grid-cell", row)[4] as HTMLElement);
    query(".grid").focus();
    fireEvent.keyDown(query(".grid"), { key: " ", code: "Space" });
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("total:")).toBeTruthy();
    fireEvent.click(within(dialog).getByText("Text"));
    expect(within(dialog).getByText(/"items": \[/)).toBeTruthy();
    press("KeyW", "w");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("edits an enum column with a dropdown of its values", async () => {
    await openTable("orders");
    fireEvent.doubleClick(await screen.findByText("new"));
    const select = await waitFor(() => query<HTMLSelectElement>(".grid-cell select"));
    const options = Array.from(select.options).map((o) => o.value);
    expect(options).toEqual(["new", "paid"]);
    fireEvent.change(select, { target: { value: "paid" } });
    expect(await screen.findByText("1 pending change")).toBeTruthy();
  });
});
