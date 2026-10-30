// @vitest-environment jsdom
// Compare schema: the target is another database; the script opens in a query tab there.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, press, query, renderApp, setupApp, tabs } from "@/test/app";

setupApp();

describe("compare schema", () => {
  it("lists the differences with another database and opens the script there", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    press("KeyP", "p");
    const input = await screen.findByPlaceholderText(/Open a table/);
    fireEvent.change(input, { target: { value: "Compare schema" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const dialog = await screen.findByRole("dialog");
    const db = within(dialog).getByLabelText("Target database");
    await within(db as HTMLElement).findByText("analytics");
    fireEvent.change(db, { target: { value: "analytics" } });
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Compare" })));
    const list = await within(dialog).findByRole("list", { name: "Differences" });
    expect(within(list).getByText("orders")).toBeTruthy();
    expect(within(list).getByText("events")).toBeTruthy();
    const script = query(".compare-script").textContent ?? "";
    expect(script).toContain('CREATE TABLE "public"."orders" (...);');
    expect(script).not.toContain("DROP TABLE");
    fireEvent.click(within(dialog).getByText(/Also drop tables/));
    expect(query(".compare-script").textContent).toContain('DROP TABLE "public"."events";');
    fireEvent.click(within(dialog).getByRole("button", { name: "Open script in target" }));
    await waitFor(() => expect(tabs().length).toBe(1));
    expect(tabs()[0]?.textContent).toContain("Sync from app");
  });
});
