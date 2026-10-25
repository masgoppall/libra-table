// @vitest-environment jsdom
// Query editor tools: Beautify, :name variables, Explain, error markers, append mode.

import { EditorView } from "@codemirror/view";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { openWorkspace, press, query, queryAll, renderApp, setupApp } from "@/test/app";
import { okResult } from "@/test/fakeObjects";

const h = setupApp();

// CodeMirror measures text ranges; jsdom has no layout for them.
beforeAll(() => {
  const rect = () => ({ x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 });
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => ({ ...rect(), toJSON: () => ({}) }) as DOMRect;
});

async function openEditor(sql: string): Promise<EditorView> {
  renderApp();
  await openWorkspace("Local Postgres");
  press("KeyT", "t");
  const dom = await waitFor(() => query(".cm-editor"));
  const view = EditorView.findFromDOM(dom);
  if (!view) throw new Error("no editor");
  act(() => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: sql } }));
  return view;
}

const ranSql = () => h.backend.callsOf("run_query").map((c) => c.args.sql);

describe("query tools", () => {
  it("beautifies the SQL with ⌥⌘F", async () => {
    const view = await openEditor("select a, b from t where id = :id");
    press("KeyF", "ƒ", { altKey: true });
    await waitFor(() =>
      expect(view.state.doc.toString()).toBe("SELECT\n  a,\n  b\nFROM\n  t\nWHERE\n  id = :id"),
    );
  });

  it("asks for variables, runs with quoted values and remembers them", async () => {
    await openEditor("SELECT * FROM people WHERE id = :id AND name = :name -- :not_me");
    press("Enter", "Enter");
    const dialog = await screen.findByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText(":id"), { target: { value: "7" } });
    fireEvent.change(within(dialog).getByLabelText(":name"), { target: { value: "O'Brien" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Run" }));
    await waitFor(() => expect(ranSql().length).toBe(1));
    expect(ranSql()[0]).toBe("SELECT * FROM people WHERE id = 7 AND name = 'O''Brien' -- :not_me");
    press("Enter", "Enter");
    const again = await screen.findByRole("dialog");
    expect((within(again).getByLabelText(":name") as HTMLInputElement).value).toBe("O'Brien");
  });

  it("explains the statement at the cursor as a plan tree", async () => {
    h.backend.objects.runQuery = (a) => [
      okResult(a.sql, {
        columns: [{ name: "QUERY PLAN", typeName: "JSON" }],
        rows: [
          [
            JSON.stringify([
              {
                Plan: {
                  "Node Type": "Seq Scan",
                  "Relation Name": "people",
                  "Total Cost": 1,
                  "Startup Cost": 0,
                },
              },
            ]),
          ],
        ],
      }),
    ];
    await openEditor("SELECT * FROM people;");
    fireEvent.click(screen.getByText("Explain"));
    await waitFor(() => expect(ranSql()).toEqual(["EXPLAIN (FORMAT JSON) SELECT * FROM people"]));
    expect(await screen.findByText("Seq Scan")).toBeTruthy();
    expect(screen.getByText("on people")).toBeTruthy();
    expect(screen.getByText("cost 0..1")).toBeTruthy();
  });

  it("marks failed statements in the gutter and appends results", async () => {
    const err = { kind: "query", layer: "database", message: "relation nope does not exist" };
    h.backend.objects.runQuery = (a) =>
      a.sql
        .split(";")
        .map((s: string, i: number) => okResult(s.trim(), i === 1 ? { error: err as never } : {}));
    await openEditor("SELECT 1;\nSELECT * FROM nope;");
    press("Enter", "Enter", { shiftKey: true });
    await waitFor(() => expect(queryAll(".cm-run-error").length).toBe(1));
    expect(query(".cm-run-error").title).toBe("relation nope does not exist");

    h.backend.objects.runQuery = (a) => [okResult(a.sql)];
    fireEvent.click(screen.getByRole("button", { name: /Append/ }));
    press("Enter", "Enter", { shiftKey: true });
    await waitFor(() => expect(queryAll(".result-tab").length).toBe(3));
    fireEvent.click(screen.getByRole("button", { name: /Clear/ }));
    await waitFor(() => expect(queryAll(".result-tab").length).toBe(0));
  });
});

describe("editor keys", () => {
  it("comments and uncomments lines with ⌘/", async () => {
    const view = await openEditor("SELECT 1");
    view.contentDOM.focus();
    press("Slash", "/");
    await waitFor(() => expect(view.state.doc.toString()).toBe("-- SELECT 1"));
    press("Slash", "/");
    await waitFor(() => expect(view.state.doc.toString()).toBe("SELECT 1"));
    // The editor took the key: the shortcuts sheet (also ⌘/) stays closed.
    expect(screen.queryByText("Keyboard shortcuts")).toBeNull();
  });
});
