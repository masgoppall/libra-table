// @vitest-environment jsdom
// Duplicate (⌘D), paste (⌘V) and a double-click below the rows, as in TablePlus.

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
const inserted = () => queryAll<HTMLElement>(".grid-row.inserted");
/** Cell `c` of a row (0 is the row number). */
const cell = (row: HTMLElement | undefined, c: number) =>
  queryAll<HTMLElement>(".grid-cell", row as HTMLElement)[c] as HTMLElement;
const gridKey = (key: string) => fireEvent.keyDown(query(".grid"), { key, metaKey: true });

async function commit() {
  press("KeyS", "s");
  const dialog = await screen.findByRole("dialog");
  await act(async () => {
    fireEvent.click(within(dialog).getByRole("button", { name: "Commit" }));
  });
  await waitFor(() => expect(h.backend.callsOf("commit_changes").length).toBe(1));
  return h.backend.lastCall("commit_changes")?.args.changes;
}

describe("row copies in the data tab", () => {
  it("duplicates the selected row with ⌘D, leaving the key to the database", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(rows()[0], 2));
    press("KeyD", "d");
    await waitFor(() => expect(inserted().length).toBe(1));
    expect(cell(inserted()[0], 2).textContent).toBe("Alice");
    expect(await commit()).toEqual([
      { kind: "insert", values: { name: "Alice", note: "likes 50%_off deals" } },
    ]);
  });

  it("adds a row on a double-click below the rows", async () => {
    await openPeople();
    fireEvent.doubleClick(query(".grid"));
    await waitFor(() => expect(inserted().length).toBe(1));
    // Not on a row: that edits the cell instead.
    fireEvent.doubleClick(cell(rows()[0], 2));
    expect(inserted().length).toBe(1);
  });

  it("pastes copied rows as new rows, keeping NULL", async () => {
    await openPeople();
    fireEvent.mouseDown(cell(rows()[1], 2));
    gridKey("c");
    await waitFor(() => expect(h.backend.clipboard).toBe("2\tBob\t"));
    await act(async () => gridKey("v"));
    await waitFor(() => expect(inserted().length).toBe(1));
    expect(await commit()).toEqual([{ kind: "insert", values: { name: "Bob", note: null } }]);
  });

  it("pastes one value into the focused cell and a block from it", async () => {
    await openPeople();
    h.backend.clipboard = "Carol";
    fireEvent.mouseDown(cell(rows()[0], 2));
    await act(async () => gridKey("v"));
    await waitFor(() => expect(cell(rows()[0], 2).textContent).toBe("Carol"));

    h.backend.clipboard = "Dan\tnew note\nEve\tother";
    fireEvent.mouseDown(cell(rows()[0], 2));
    await act(async () => gridKey("v"));
    await waitFor(() => expect(cell(rows()[1], 3).textContent).toBe("other"));
    expect(cell(rows()[0], 2).textContent).toBe("Dan");
    expect(cell(rows()[1], 2).textContent).toBe("Eve");
    expect(inserted().length).toBe(0);
  });
});
