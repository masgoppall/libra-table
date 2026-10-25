// @vitest-environment jsdom
// Metrics board: create a board, add widgets, an input variable feeds another widget.

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { openWorkspace, query, renderApp, setupApp, sidebar } from "@/test/app";

const h = setupApp();

async function addWidget(kind: string) {
  fireEvent.click(screen.getByText("Add widget"));
  fireEvent.click(await screen.findByText(kind));
  return screen.findByRole("dialog");
}

describe("metrics board", () => {
  it("builds a board with a scoreboard and a chart driven by an input", async () => {
    renderApp();
    await openWorkspace("Local Postgres");
    fireEvent.click(within(sidebar()).getByText("Metrics"));
    fireEvent.click(await screen.findByRole("button", { name: "New board" }));
    const prompt = await screen.findByRole("dialog");
    fireEvent.change(within(prompt).getByDisplayValue("Dashboard"), { target: { value: "Sales" } });
    await act(async () => fireEvent.click(within(prompt).getByText("Save")));
    await waitFor(() => expect(h.backend.tools.boards[0]?.name).toBe("Sales"));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    let editor = await addWidget("Scoreboard");
    fireEvent.change(within(editor).getByLabelText("Widget query"), {
      target: { value: "SELECT count(*) FROM people" },
    });
    fireEvent.change(within(editor).getByLabelText("Widget title"), {
      target: { value: "People" },
    });
    fireEvent.click(within(editor).getByText("Save widget"));
    const card = await screen.findByRole("region", { name: "People" });
    expect(await within(card).findByText("42")).toBeTruthy();

    editor = await addWidget("Input");
    fireEvent.change(within(editor).getByLabelText("Variable name"), {
      target: { value: "region" },
    });
    fireEvent.change(within(editor).getByLabelText("Default value"), { target: { value: "EU" } });
    fireEvent.click(within(editor).getByText("Save widget"));

    editor = await addWidget("Bar chart");
    fireEvent.change(within(editor).getByLabelText("Widget query"), {
      target: { value: "SELECT month, sales FROM s WHERE region = {{region}}" },
    });
    fireEvent.click(within(editor).getByText("Save widget"));
    const runs = () =>
      h.backend.callsOf("run_metric_query").filter((c) => /region/.test(c.args.sql));
    await waitFor(() => expect(runs().length).toBeGreaterThan(0));
    expect(runs().at(-1)?.args.variables).toEqual([{ name: "region", value: "EU", kind: "text" }]);
    await waitFor(() => expect(document.querySelectorAll(".chart-bar").length).toBe(2));

    fireEvent.change(screen.getByRole("textbox", { name: "region" }), { target: { value: "US" } });
    await waitFor(() => expect(runs().at(-1)?.args.variables[0].value).toBe("US"), {
      timeout: 2000,
    });
    // Only the widget that uses the variable ran again.
    const counts = h.backend.callsOf("run_metric_query").filter((c) => /count/.test(c.args.sql));
    expect(counts.length).toBe(1);

    const saved = h.backend.tools.boards[0];
    expect(saved?.widgets.map((w) => w.kind)).toEqual(["scoreboard", "input", "bar"]);
    // Reorder and resize in edit mode are saved too.
    fireEvent.click(within(query("section[aria-label='People']")).getByLabelText("Move later"));
    fireEvent.click(within(query("section[aria-label='People']")).getByLabelText("Larger"));
    await waitFor(() =>
      expect(h.backend.tools.boards[0]?.widgets.map((w) => [w.kind, w.width])).toEqual([
        ["input", 1],
        ["scoreboard", 2],
        ["bar", 2],
      ]),
    );
  });
});
