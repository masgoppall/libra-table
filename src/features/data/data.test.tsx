// @vitest-environment jsdom
// Data tab: filters, edits, commit on production, undo.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, press, query, queryAll, renderApp, sidebar, setupApp } from "@/test/app";

const h = setupApp();

async function openPeople(connection: string) {
  renderApp();
  await openWorkspace(connection);
  fireEvent.click(within(sidebar()).getByText("people"));
}

describe("data tab", () => {
  it("sends filters as values, not SQL; Enter applies (GRD-2, FLT-1, FLT-4)", async () => {
    await openPeople("Local Postgres");
    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(screen.getByText("1–2")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Filter/ }));
    const row = query(".filter-row");
    const [colSelect, opSelect] = queryAll<HTMLSelectElement>("select", row);
    fireEvent.change(colSelect as HTMLSelectElement, { target: { value: "note" } });
    fireEvent.change(opSelect as HTMLSelectElement, { target: { value: "contains" } });
    const value = query<HTMLInputElement>("input.value", row);
    fireEvent.change(value, { target: { value: "50%_off" } });
    // Enter in a value applies the filters (it used to clear them).
    fireEvent.keyDown(value, { key: "Enter" });
    await waitFor(() => expect(h.backend.callsOf("fetch_page").length).toBeGreaterThan(1));
    const last = h.backend.lastCall("fetch_page")?.args;
    expect(last?.request.filters[0]).toMatchObject({
      column: "note",
      operator: "contains",
      value: "50%_off",
      enabled: true,
    });
    expect(last?.request.applyOnly).toBeNull();
    expect(last?.database).toBeNull();
  });

  it("shows every row again on an emptied value, a removed row or Clear", async () => {
    await openPeople("Local Postgres");
    expect(await screen.findByText("Alice")).toBeTruthy();
    const pages = () => h.backend.callsOf("fetch_page").length;
    const active = () =>
      (h.backend.lastCall("fetch_page")?.args.request.filters as { enabled: boolean }[]).filter(
        (f) => f.enabled,
      ).length;
    const applyValue = async (text: string) => {
      const before = pages();
      const value = query<HTMLInputElement>(".filter-row input.value");
      fireEvent.change(value, { target: { value: text } });
      fireEvent.keyDown(value, { key: "Enter" });
      await waitFor(() => expect(pages()).toBeGreaterThan(before));
    };

    fireEvent.click(screen.getByRole("button", { name: /Filter/ }));
    await applyValue("Alice");
    expect(active()).toBe(1);
    // Emptying the value and pressing ↵ drops the filter instead of failing.
    await applyValue("");
    expect(active()).toBe(0);

    await applyValue("Alice");
    let before = pages();
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(pages()).toBeGreaterThan(before));
    expect(active()).toBe(0);
    expect(query<HTMLInputElement>(".filter-row input.value").value).toBe("");

    await applyValue("Alice");
    before = pages();
    fireEvent.click(screen.getByTitle("Remove this filter"));
    await waitFor(() => expect(pages()).toBeGreaterThan(before));
    expect(active()).toBe(0);
    expect(document.querySelector(".filters")).toBeNull();
  });

  it("shows the structure in the same tab from the toolbar", async () => {
    await openPeople("Local Postgres");
    expect(await screen.findByText("Alice")).toBeTruthy();
    fireEvent.click(within(query(".toolbar")).getByRole("button", { name: /Structure/ }));
    expect(await screen.findByText("column_name")).toBeTruthy();
    expect(queryAll(".tabbar .tab").length).toBe(1);
    press("KeyS", "S", { shiftKey: true });
    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(queryAll(".tabbar .tab").length).toBe(1);
  });

  it("requires confirmation before committing edits on production (GRD-4, SAFE-2)", async () => {
    await openPeople("Prod Shop");
    fireEvent.doubleClick(await screen.findByText("Bob"));
    const editor = query<HTMLInputElement>(".grid-cell input");
    fireEvent.change(editor, { target: { value: "Robert" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    expect(await screen.findByText("1 pending change")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^Commit$/ }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Production: Prod Shop/)).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(dialog).getByText("Commit on production"));
    });
    await waitFor(() => expect(h.backend.callsOf("commit_changes").length).toBe(1));
    const commit = h.backend.lastCall("commit_changes")?.args;
    expect(commit?.confirmed).toBe(true);
    expect(commit?.changes).toEqual([
      { kind: "update", pk: { id: 2 }, values: { name: "Robert" } },
    ]);
  });

  it("keeps NULL when a NULL cell is opened and left empty, and ⌘Z undoes an edit", async () => {
    await openPeople("Local Postgres");
    await screen.findByText("Bob");
    const bobRow = queryAll(".grid-row")[1] as HTMLElement;
    const nullCell = queryAll(".grid-cell", bobRow)[3] as HTMLElement;
    expect(nullCell.textContent).toBe("NULL");
    fireEvent.doubleClick(nullCell);
    fireEvent.blur(query(".grid-cell input"));
    expect(screen.queryByText(/pending change/)).toBeNull();

    fireEvent.doubleClick(screen.getByText("Alice"));
    const editor = query<HTMLInputElement>(".grid-cell input");
    fireEvent.change(editor, { target: { value: "Alicia" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    expect(await screen.findByText("1 pending change")).toBeTruthy();
    query(".grid").focus();
    press("KeyZ", "z");
    await waitFor(() => expect(screen.queryByText(/pending change/)).toBeNull());
  });
});
