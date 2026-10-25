// @vitest-environment jsdom
// Redis workspace: databases, key list with pattern and type filter, the value panel
// (hash edit, TTL), deleting keys and running a command.

import { EditorView } from "@codemirror/view";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it } from "vitest";
import { press, query, queryAll, renderApp, setupApp, sidebar } from "@/test/app";
import { FakeRedis } from "@/test/fakeRedis";

const h = setupApp();
let redis = new FakeRedis();

beforeEach(() => {
  redis = new FakeRedis();
  redis.install(h.backend);
});

// CodeMirror measures text ranges; jsdom has no layout for them.
beforeAll(() => {
  const rect = () => ({ x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 });
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => ({ ...rect(), toJSON: () => ({}) }) as DOMRect;
});

async function openRedis() {
  renderApp();
  fireEvent.doubleClick(await screen.findByText("Local Redis"));
  await screen.findByText("user:1");
}

const keyCells = () => queryAll(".kv-keys .grid-row").map((r) => queryAll(".grid-cell", r)[1]);
const keysShown = () => keyCells().map((c) => c?.textContent);
const lastEdit = () => h.backend.lastCall("redis_edit")?.args;

async function openKey(name: string) {
  fireEvent.mouseDown(await screen.findByText(name));
  return waitFor(() => query(".kv-panel"));
}

describe("Redis workspace", () => {
  it("lists databases with key counts and the keys of db0", async () => {
    await openRedis();
    const side = within(sidebar());
    const dbs = () => queryAll(".side-item.db", sidebar()).map((d) => d.textContent);
    // Empty databases are hidden until asked for; counts come with the names.
    expect(dbs()).toEqual(["db04", "db27"]);
    fireEvent.click(side.getByTitle("Show empty databases"));
    expect(dbs()).toEqual(["db04", "db10", "db27"]);
    expect(keysShown().sort()).toEqual(["queue", "session:abc", "user:1", "user:2"]);
    expect(screen.getByText(/4 keys in the database/)).toBeTruthy();
    // The tables UI of SQL drivers isn't there.
    expect(side.queryByText("Tables")).toBeNull();
  });

  it("filters keys by pattern and type", async () => {
    await openRedis();
    const input = screen.getByLabelText("Key pattern");
    fireEvent.change(input, { target: { value: "user:*" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(keysShown().sort()).toEqual(["user:1", "user:2"]));
    expect(h.backend.lastCall("redis_scan")?.args.request).toMatchObject({
      pattern: "user:*",
      keyType: "",
      page: 0,
    });
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(screen.getByLabelText("Key type"), { target: { value: "list" } });
    await waitFor(() => expect(keysShown()).toEqual(["queue"]));
    expect(h.backend.lastCall("redis_scan")?.args.request.keyType).toBe("list");
  });

  it("opens a hash and edits a field", async () => {
    await openRedis();
    const panel = await openKey("user:1");
    await within(panel).findByText("Ada");
    expect(within(panel).getByText("hash")).toBeTruthy();
    fireEvent.doubleClick(within(panel).getByText("Ada"));
    const input = within(panel).getByDisplayValue("Ada");
    fireEvent.change(input, { target: { value: "Grace" } });
    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    await waitFor(() =>
      expect(lastEdit()?.edit).toEqual({
        op: "hash_set",
        key: "user:1",
        field: "name",
        value: "Grace",
      }),
    );
    expect(lastEdit()?.confirmed).toBe(false);
    await within(panel).findByText("Grace");

    // A new field from the row at the bottom.
    fireEvent.change(within(panel).getByLabelText("New field"), { target: { value: "city" } });
    fireEvent.change(within(panel).getByLabelText("New value"), { target: { value: "London" } });
    fireEvent.click(within(panel).getByText("Add field"));
    await within(panel).findByText("London");
    expect(lastEdit()?.edit).toMatchObject({ op: "hash_set", field: "city", value: "London" });
  });

  it("sets and removes a TTL", async () => {
    await openRedis();
    const panel = await openKey("user:2");
    await within(panel).findByText("No expiry");
    fireEvent.click(within(panel).getByText("Set TTL"));
    const dialog = await screen.findByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("TTL (seconds)"), { target: { value: "90" } });
    await act(async () => {
      fireEvent.click(within(dialog).getByText("Set TTL"));
    });
    await waitFor(() =>
      expect(lastEdit()?.edit).toEqual({ op: "expire", key: "user:2", seconds: 90 }),
    );
    await within(panel).findByText("90 s");
  });

  it("deletes the selected key after confirming", async () => {
    await openRedis();
    fireEvent.mouseDown(screen.getByText("queue"));
    fireEvent.click(screen.getByRole("button", { name: /Delete/ }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("queue")).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    });
    await waitFor(() => expect(lastEdit()?.edit).toEqual({ op: "delete", keys: ["queue"] }));
    expect(lastEdit()?.confirmed).toBe(true);
    await waitFor(() => expect(keysShown()).not.toContain("queue"));
  });

  it("runs a command in the query editor", async () => {
    await openRedis();
    press("KeyT", "t");
    const dom = await waitFor(() => query(".cm-editor"));
    const view = EditorView.findFromDOM(dom);
    if (!view) throw new Error("no editor");
    act(() => view.dispatch({ changes: { from: 0, insert: "GET session:abc" } }));
    // The command word is highlighted.
    await waitFor(() => expect(query(".cm-redis-command").textContent).toBe("GET"));
    press("Enter", "Enter");
    await waitFor(() => expect(redis.ran).toEqual(["GET session:abc"]));
    expect(h.backend.lastCall("run_query")?.args.sql).toBe("GET session:abc");
    const pane = () => queryAll(".tab-pane").find((p) => p.style.display === "flex");
    await waitFor(() => expect(pane()?.textContent).toContain('{"uid":1}'));
    expect(pane()?.querySelector(".kv-keys")).toBeNull();
  });
});

describe("Redis on production", () => {
  it("asks before an edit and resends it confirmed", async () => {
    renderApp();
    fireEvent.doubleClick(await screen.findByText("Prod Redis"));
    const panel = await openKey("user:1");
    fireEvent.doubleClick(await within(panel).findByText("Ada"));
    const input = within(panel).getByDisplayValue("Ada");
    fireEvent.change(input, { target: { value: "Grace" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Production: Prod Redis/)).toBeTruthy();
    expect(within(dialog).getByText("HSET user:1 name Grace")).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(dialog).getByText("Run on production"));
    });
    await waitFor(() => expect(redis.edits.length).toBe(1));
    expect(h.backend.callsOf("redis_edit").map((c) => c.args.confirmed)).toEqual([false, true]);
  });
});
