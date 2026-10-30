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
      "Move Tab to New Window",
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

  it("shows opened databases on a rail: switch, follow the tab, close", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(within(sidebar()).getByText("people"));
    await screen.findByText("Alice");
    expect(document.querySelector(".db-rail")).toBeNull();

    press("KeyK", "k");
    await within(query(".dbs-sheet")).findByText("analytics");
    fireEvent.doubleClick(within(query(".dbs-sheet")).getByText("analytics"));
    const rail = () => query(".db-rail");
    const shown = () => within(rail()).getByRole("tab", { selected: true }).textContent;
    await waitFor(() => expect(shown()).toBe("analytics"));
    fireEvent.click(await within(sidebar()).findByText("events"));
    await waitFor(() => expect(tabs().length).toBe(2));

    // The people tab brings the sidebar back to its database.
    fireEvent.mouseDown(tabAt(0));
    await waitFor(() => expect(shown()).toBe("app"));
    fireEvent.click(within(rail()).getByRole("tab", { name: "analytics" }));
    await waitFor(() => expect(shown()).toBe("analytics"));

    fireEvent.contextMenu(within(rail()).getByRole("tab", { name: "analytics" }));
    fireEvent.click(await screen.findByText("Close database"));
    await waitFor(() => expect(document.querySelector(".db-rail")).toBeNull());
    // One database left: the tab no longer names it.
    expect(tabs().map((t) => t.textContent)).toEqual(["people"]);
  });

  it("opens another connection from inside one with ⇧⌘K", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyK", "K", { shiftKey: true });
    const sheet = await screen.findByRole("dialog", { name: "Open connection" });
    expect(within(sheet).getByText("Local Postgres")).toBeTruthy();
    fireEvent.change(within(sheet).getByPlaceholderText("Search for connection..."), {
      target: { value: "prod" },
    });
    fireEvent.doubleClick(await within(sheet).findByText("Prod Shop"));
    await waitFor(() => expect(document.querySelectorAll(".conn-tab").length).toBe(2));
    expect(screen.queryByRole("dialog", { name: "Open connection" })).toBeNull();
  });
});

describe("tables opened in place", () => {
  it("replaces the table tab and goes back and forward", async () => {
    h.backend.settings = { ...h.backend.settings, openInNewTab: false };
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(query('.side-item[title="people"]'));
    await screen.findByText("Alice");
    fireEvent.click(query('.side-item[title="orders"]'));
    await waitFor(() => expect(tabAt(0).textContent).toContain("orders"));
    expect(tabs().length).toBe(1);

    press("BracketLeft", "[", { altKey: true });
    await waitFor(() => expect(tabAt(0).textContent).toContain("people"));
    await screen.findByText("Alice");
    fireEvent.click(screen.getByRole("button", { name: "Forward" }));
    await waitFor(() => expect(tabAt(0).textContent).toContain("orders"));

    // ⌘-click still opens a tab of its own.
    fireEvent.click(query('.side-item[title="people"]'), { metaKey: true, ctrlKey: true });
    await waitFor(() => expect(tabs().length).toBe(2));
  });
});

describe("reconnect", () => {
  it("reconnects with ⇧⌘R and keeps the tabs", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(query('.side-item[title="people"]'));
    await screen.findByText("Alice");
    press("KeyR", "r", { shiftKey: true });
    await waitFor(() => expect(h.backend.callsOf("connect").length).toBe(2));
    expect(h.backend.callsOf("disconnect").length).toBe(1);
    expect(tabs().length).toBe(1);
    await screen.findByText("Reconnected.");
  });
});

describe("windows", () => {
  it("moves a tab to a new window of the same connection", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(query('.side-item[title="people"]'));
    await screen.findByText("Alice");
    fireEvent.contextMenu(tabAt(0));
    fireEvent.click(screen.getByText("Move Tab to New Window"));
    await waitFor(() => expect(tabs().length).toBe(0));
    const args = h.backend.lastCall("open_window")?.args;
    expect(args?.id).toBe("c-local");
    const tab = JSON.parse(args?.tab);
    expect(tab).toMatchObject({ kind: "data", table: "people" });
    expect(tab.id).toBeUndefined();
  });

  it("opens the connection in a new window with ⌥⌘N", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyN", "n", { altKey: true });
    await waitFor(() => expect(h.backend.lastCall("open_window")?.args.tab).toBeNull());
  });
});
