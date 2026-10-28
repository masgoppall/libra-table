// @vitest-environment jsdom
// Workspace: tab shortcuts, the ⌘P palette, toolbar and sidebar tabs, several databases.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, press, query, renderApp, sidebar, tabAt, tabs, setupApp } from "@/test/app";

const h = setupApp();

describe("workspace", () => {
  it("opens and closes tabs with ⌘T / ⌘W and switches with ⌘1", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    expect(tabs().length).toBe(0);
    press("KeyT", "t");
    await waitFor(() => expect(tabs().length).toBe(1));
    press("KeyT", "t");
    await waitFor(() => expect(tabs().length).toBe(2));
    expect(tabAt(1).className).toContain("active");
    press("Digit1", "1");
    await waitFor(() => expect(tabAt(0).className).toContain("active"));
    press("KeyW", "w");
    await waitFor(() => expect(tabs().length).toBe(1));
  });

  it("opens a table from the ⌘P palette", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyP", "p");
    const input = await screen.findByPlaceholderText(/Open a table/);
    await within(query(".palette")).findByText("active_people");
    fireEvent.change(input, { target: { value: "peop" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByText("Alice");
    expect(tabAt(0).textContent).toContain("people");
  });

  it("shows the connection in the toolbar and switches the sidebar's tabs", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    expect(query(".tb-status-text").textContent).toBe(
      "LOCAL | Local Postgres : PostgreSQL 16.4 : app",
    );
    const side = within(sidebar());
    expect(side.getByPlaceholderText("Search for item...")).toBeTruthy();
    fireEvent.click(side.getByRole("tab", { name: "Queries" }));
    expect(await side.findByText(/No saved queries yet/)).toBeTruthy();
    fireEvent.click(side.getByRole("tab", { name: "History" }));
    expect(side.getByPlaceholderText("Search for history...")).toBeTruthy();
    fireEvent.click(side.getByRole("tab", { name: "Items" }));
    expect(await side.findByText("people")).toBeTruthy();
  });

  it("has TablePlus' tab menu and connection info on the status pill", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyT", "t");
    await waitFor(() => expect(tabs().length).toBe(1));
    fireEvent.contextMenu(tabAt(0));
    const labels = Array.from(document.querySelectorAll(".menu .menu-label")).map(
      (l) => l.textContent,
    );
    expect(labels).toEqual([
      "New Tab",
      "Close Tab",
      "Close other Tabs",
      "Close Tabs to the Right",
      "Close all Tabs",
      "Rename Tab",
    ]);
    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByRole("button", { name: "Connection info" })).toBeNull();
    fireEvent.click(query(".tb-status"));
    const info = await screen.findByRole("dialog", { name: "Connection info" });
    expect(within(info).getByText("Local Postgres")).toBeTruthy();
    expect(within(info).getByRole("button", { name: "Copy URL" })).toBeTruthy();
  });

  it("works with several databases at once (⌘K switch, tabs keep their database)", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(within(sidebar()).getByText("people"));
    await screen.findByText("Alice");

    press("KeyK", "k");
    const input = await screen.findByPlaceholderText("Search for database...");
    await within(query(".dbs-sheet")).findByText("analytics");
    fireEvent.change(input, { target: { value: "analy" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(await within(sidebar()).findByText("events"));
    const eventsPage = () =>
      h.backend.callsOf("fetch_page").find((c) => c.args.request.table === "events");
    await waitFor(() => expect(eventsPage()).toBeTruthy());
    expect(eventsPage()?.args.database).toBe("analytics");
    expect(tabs().map((t) => t.textContent)).toEqual(["peopleapp", "eventsanalytics"]);

    // The first tab still reads from the connection's own database.
    const before = h.backend.callsOf("fetch_page").length;
    fireEvent.mouseDown(tabAt(0));
    press("KeyR", "r");
    await waitFor(() => expect(h.backend.callsOf("fetch_page").length).toBeGreaterThan(before));
    expect(h.backend.lastCall("fetch_page")?.args).toMatchObject({
      database: null,
      request: { table: "people" },
    });
  });
});
