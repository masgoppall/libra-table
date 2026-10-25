import { describe, expect, it } from "vitest";
import { type Workspace, type WorkspaceTab } from "@/types";
import * as m from "./workspaceModel";

const tab = (id: string, extra: Partial<WorkspaceTab> = {}): WorkspaceTab => ({
  id,
  kind: "query",
  title: id,
  database: "app",
  ...extra,
});

function ws(tabs: WorkspaceTab[], activeTabId: string | null = tabs[0]?.id ?? null): Workspace {
  return {
    connectionId: "c",
    info: {
      connectionId: "c",
      driver: "postgres",
      database: "app",
      serverVersion: "16",
      throughTunnel: false,
    },
    caps: { operators: [], hasSchemas: true, switchDatabase: true },
    database: "app",
    schema: "public",
    tabs,
    activeTabId,
    refreshKey: 0,
  };
}

describe("workspace model", () => {
  it("opens a tab next to the active one", () => {
    const w = ws([tab("a"), tab("b")], "a");
    const { tabs, activeTabId } = m.openTab(w, { kind: "query", title: "new" }, false);
    expect(tabs.map((t) => t.title)).toEqual(["a", "new", "b"]);
    expect(activeTabId).toBe(tabs[1]?.id);
    expect(tabs[1]?.database).toBe("app");
  });

  it("reuses the data tab of the same table", () => {
    const data = tab("d", { kind: "data", schema: "public", table: "people" });
    const w = ws([tab("a"), data], "a");
    const next = m.openTab(
      w,
      { kind: "data", title: "people", schema: "public", table: "people" },
      true,
    );
    expect(next.tabs).toHaveLength(2);
    expect(next.activeTabId).toBe("d");
  });

  it("activates a neighbour when the active tab closes", () => {
    const w = ws([tab("a"), tab("b"), tab("c")], "b");
    expect(m.closeTab(w, "b").activeTabId).toBe("c");
    expect(m.closeTab(ws([tab("a"), tab("b")], "b"), "b").activeTabId).toBe("a");
    expect(m.closeTab(w, "a").activeTabId).toBe("b");
  });

  it("closes the tabs to the right and keeps a valid active tab", () => {
    const w = ws([tab("a"), tab("b"), tab("c")], "c");
    expect(m.closeTabsAfter(w, "a")).toEqual({ tabs: [w.tabs[0]], activeTabId: "a" });
  });

  it("moves and cycles tabs", () => {
    const w = ws([tab("a"), tab("b"), tab("c")], "c");
    expect(m.moveTab(w, "c", 0).map((t) => t.id)).toEqual(["c", "a", "b"]);
    expect(m.cycleTab(w, 1)).toBe("a");
    expect(m.cycleTab(w, -1)).toBe("b");
    expect(m.tabByNumber(w, 9)?.id).toBe("c");
  });

  it("picks the next view after closing a connection", () => {
    expect(m.viewAfterClose(["x", "y", "z"], "y", "y")).toBe("z");
    expect(m.viewAfterClose(["x", "y"], "y", "y")).toBe("x");
    expect(m.viewAfterClose(["y"], "y", "y")).toBe("home");
    expect(m.viewAfterClose(["x", "y"], "y", "x")).toBe("x");
  });

  it("restores saved tabs, giving old tabs the connection's database", () => {
    const w = ws([]);
    const saved = {
      schema: "sales",
      tabs: [{ id: "t", kind: "query" as const, title: "q" }],
      activeTabId: "t",
    };
    const restored = m.restoreWorkspace(w.info, w.caps, saved);
    expect(restored.schema).toBe("sales");
    expect(restored.tabs[0]?.database).toBe("app");
  });

  it("does not save new-table drafts", () => {
    const w = ws([tab("a", { kind: "new-table" }), tab("b")], "a");
    expect(m.toSaved(w)).toMatchObject({ tabs: [{ id: "b" }], activeTabId: "b" });
  });

  it("sends null for the connection's own database", () => {
    const w = ws([]);
    expect(m.tabDb(w, { database: "app" })).toBeNull();
    expect(m.tabDb(w, { database: "analytics" })).toBe("analytics");
  });
});
