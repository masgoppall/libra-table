// @vitest-environment jsdom
// Blocks of cells in the grid: drag or Shift+←/→ selects them, ⌘C copies only those cells.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, press, query, queryAll, renderApp, setupApp, sidebar } from "@/test/app";

const h = setupApp();

async function openPeople() {
  renderApp();
  await openWorkspace("Local Postgres");
  fireEvent.click(within(sidebar()).getByText("people"));
  await screen.findByText("Alice");
}

const rows = () => queryAll<HTMLElement>(".grid-row");
const cell = (row: number, c: number) =>
  queryAll<HTMLElement>(".grid-cell", rows()[row] as HTMLElement)[c] as HTMLElement;
const gridKey = (key: string, mods: { metaKey?: boolean; shiftKey?: boolean } = {}) =>
  fireEvent.keyDown(query(".grid"), { key, ...mods });

describe("blocks of cells", () => {
  it("drags across cells and copies just those", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(0, 1));
    fireEvent.mouseEnter(cell(1, 2));
    fireEvent.mouseUp(window);
    expect(queryAll(".grid-cell.in-range").length).toBe(4);
    gridKey("c", { metaKey: true });
    await waitFor(() => expect(h.backend.clipboard).toBe("1\tAlice\n2\tBob"));
  });

  it("grows a block with Shift+arrows and fills it with one pasted value", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(0, 2));
    fireEvent.mouseUp(window);
    gridKey("ArrowRight", { shiftKey: true });
    gridKey("ArrowDown", { shiftKey: true });
    expect(queryAll(".grid-cell.in-range").length).toBe(4);
    h.backend.clipboard = "x";
    await act(async () => gridKey("v", { metaKey: true }));
    await waitFor(() => expect(cell(1, 3).textContent).toBe("x"));
    expect([cell(0, 2), cell(0, 3), cell(1, 2)].map((c) => c.textContent)).toEqual(["x", "x", "x"]);
  });

  it("offers Copy cells as in the menu of a block", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(0, 1));
    fireEvent.mouseEnter(cell(1, 2));
    fireEvent.mouseUp(window);
    fireEvent.contextMenu(cell(1, 1));
    fireEvent.mouseEnter(screen.getByText("Copy cells as"));
    fireEvent.click(screen.getByText("JSON"));
    await waitFor(() =>
      expect(JSON.parse(h.backend.clipboard)).toEqual([
        { id: 1, name: "Alice" },
        { id: 2, name: "Bob" },
      ]),
    );
  });
});

describe("jump to column or row", () => {
  it("moves the focus to the column picked with ⌘J", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(1, 1));
    fireEvent.mouseUp(window);
    press("KeyJ", "j");
    const input = await screen.findByPlaceholderText("Column, row number, or value");
    fireEvent.change(input, { target: { value: "note" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(cell(1, 3).className).toContain("focus"));
  });
});

describe("search and replace within the table", () => {
  it("highlights matches and replaces them all as pending edits", async () => {
    await openPeople();
    press("KeyR", "r", { altKey: true });
    const search = await screen.findByLabelText("Search within table");
    fireEvent.change(search, { target: { value: "li" } });
    expect(queryAll(".grid-cell.match").length).toBe(2);
    expect(screen.getByText("2 matches")).toBeTruthy();
    fireEvent.keyDown(search, { key: "Enter" });
    expect(screen.getByText("1 of 2")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Replace with"), { target: { value: "LI" } });
    fireEvent.click(screen.getByRole("button", { name: "Replace All (2)" }));
    expect(cell(0, 2).textContent).toBe("ALIce");
    expect(cell(0, 3).textContent).toBe("LIkes 50%_off deals");
    expect(queryAll(".grid-cell.edited").length).toBe(2);
  });
});

describe("copy script as", () => {
  it("copies the table's CREATE statement from the sidebar", async () => {
    await openPeople();
    fireEvent.contextMenu(query('.side-item[title="orders"]'));
    fireEvent.mouseEnter(screen.getByText("Copy script as"));
    fireEvent.click(screen.getByText("CREATE"));
    await waitFor(() => expect(h.backend.clipboard).toBe('CREATE TABLE "public"."orders" (...)'));
  });
});

describe("saved query folders", () => {
  it("groups saved queries by folder and moves one out", async () => {
    h.backend.savedQueries = [
      { name: "Loose", modifiedAt: 0, size: 1 },
      { name: "Reports/Monthly", modifiedAt: 0, size: 1 },
    ];
    await openPeople();
    fireEvent.click(within(sidebar()).getByText("Queries"));
    const folder = await within(sidebar()).findByText("Reports");
    expect(within(sidebar()).getByText("Monthly")).toBeTruthy();
    fireEvent.click(folder);
    expect(within(sidebar()).queryByText("Monthly")).toBeNull();
    fireEvent.click(folder);
    fireEvent.contextMenu(within(sidebar()).getByText("Monthly"));
    fireEvent.click(screen.getByText("Move to folder…"));
    const prompt = await screen.findByRole("dialog");
    fireEvent.change(within(prompt).getByDisplayValue("Reports"), { target: { value: "" } });
    await act(async () => fireEvent.click(within(prompt).getByText("OK")));
    await waitFor(() =>
      expect(h.backend.lastCall("rename_saved_query")?.args).toMatchObject({
        from: "Reports/Monthly",
        to: "Monthly",
      }),
    );
  });
});
