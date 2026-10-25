// @vitest-environment jsdom
// Schema objects in the sidebar: definition tab, edit script, drop, pins and recent items.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, query, renderApp, sidebar, tabs, setupApp } from "@/test/app";
import { functionDdl } from "@/test/fakeObjects";

const h = setupApp();

describe("schema objects", () => {
  it("opens a function's definition and its edit script", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    const side = within(sidebar());
    expect(await side.findByText("Functions")).toBeTruthy();
    expect(side.getByText("Materialized views")).toBeTruthy();
    fireEvent.click(side.getByText("add"));
    await waitFor(() => expect(query(".definition-ddl").textContent).toBe(functionDdl("add")));
    expect(h.backend.lastCall("object_definition")?.args).toMatchObject({
      schema: "public",
      object: { kind: "function", name: "add", arguments: "a integer, b integer" },
    });
    fireEvent.click(screen.getByText("Edit in query tab"));
    await waitFor(() => expect(tabs().map((t) => t.textContent)).toContain("Edit · add"));
  });

  it("drops a function after the preview and typing its name", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(await within(sidebar()).findByText("add"));
    fireEvent.click(await screen.findByText("Drop function…"));
    fireEvent.click(within(await screen.findByRole("dialog")).getByText("Continue"));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("-- drop_object")).toBeTruthy();
    const ok = within(dialog).getByRole("button", { name: "Drop function" }) as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "add" } });
    await act(async () => {
      fireEvent.click(ok);
    });
    await waitFor(() => expect(h.backend.callsOf("apply_schema_change").length).toBe(1));
    expect(h.backend.lastCall("apply_schema_change")?.args.change).toEqual({
      kind: "drop_object",
      schema: "public",
      object: { kind: "function", name: "add", arguments: "a integer, b integer", table: null },
      cascade: false,
    });
  });

  it("pins a table and remembers what was opened", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("people"));
    fireEvent.click(await screen.findByText("Pin to top"));
    await waitFor(() => expect(sidebar().querySelector(".side-pin")).toBeTruthy());
    expect(h.backend.objects.sidebar.pinned).toEqual([
      { database: "app", schema: "public", kind: "table", name: "people" },
    ]);

    fireEvent.click(within(sidebar()).getByText("active_people"));
    await waitFor(() => expect(h.backend.objects.sidebar.recent.length).toBe(1));
    expect(h.backend.objects.sidebar.recent[0]).toMatchObject({
      kind: "view",
      name: "active_people",
    });
    // "Recent" starts collapsed; opened, it lists the view again.
    fireEvent.click(within(sidebar()).getByText("Recent"));
    expect(within(sidebar()).getAllByText("active_people").length).toBe(2);
  });
});
