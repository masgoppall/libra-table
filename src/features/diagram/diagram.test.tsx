// @vitest-environment jsdom
// ER diagram: open it from the sidebar, see tables and relations, open a table.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, query, renderApp, setupApp, sidebar, tabs } from "@/test/app";

const h = setupApp();

describe("ER diagram", () => {
  it("shows the schema's tables and foreign keys and opens a table", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.contextMenu(within(sidebar()).getByText("Tables"));
    fireEvent.click(await screen.findByText("Open ER diagram"));
    await waitFor(() => expect(document.querySelector("[data-table='orders']")).toBeTruthy());
    expect(h.backend.lastCall("schema_diagram")?.args).toMatchObject({ schema: "public" });
    expect(document.querySelectorAll(".diagram-edge").length).toBe(1);
    expect(screen.getByText("2 tables · 1 relationship")).toBeTruthy();
    const people = query("[data-table='people']");
    // Hovering highlights the relation.
    fireEvent.mouseEnter(people);
    expect(query(".diagram-edge").getAttribute("class")).toContain("focus");
    // Dragging a table saves its position for this schema.
    fireEvent.mouseDown(people, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(window, { clientX: 60, clientY: 30 });
    fireEvent.mouseUp(window);
    await waitFor(() => expect(h.backend.callsOf("save_diagram_layout").length).toBe(1));
    expect(h.backend.lastCall("save_diagram_layout")?.args.key).toBe("app/public");
    fireEvent.doubleClick(people);
    await waitFor(() => expect(tabs().some((t) => t.textContent?.includes("people"))).toBe(true));
  });
});
