import { describe, expect, it } from "vitest";
import { type Workspace, type WorkspaceTab } from "@/types";
import { pgCapabilities } from "@/test/drivers";
import { navigateTab, stepTab } from "./tabHistory";

const people: WorkspaceTab = {
  id: "t",
  kind: "data",
  title: "people",
  database: "app",
  schema: "public",
  table: "people",
  pendingFilter: { column: "id", value: "1" },
};

const ws = (tabs: WorkspaceTab[]): Workspace => ({
  connectionId: "c",
  info: {
    connectionId: "c",
    driver: "postgres",
    dialect: "postgres",
    database: "app",
    serverVersion: "16",
    throughTunnel: false,
  },
  caps: pgCapabilities(),
  database: "app",
  schema: "public",
  tabs,
  activeTabId: tabs[0]?.id ?? null,
  refreshKey: 0,
});

const orders = { kind: "data" as const, title: "orders", schema: "public", table: "orders" };

describe("tab history", () => {
  it("opens another table in the tab and steps back and forward", () => {
    const opened = navigateTab(ws([people]), "t", orders);
    const t = opened.tabs?.[0];
    expect(t?.table).toBe("orders");
    expect(t?.database).toBe("app");
    expect(t?.pendingFilter).toBeUndefined();
    expect(t?.back?.map((p) => p.table)).toEqual(["people"]);

    const back = stepTab(ws(opened.tabs ?? []), "t", -1).tabs?.[0];
    expect(back?.table).toBe("people");
    expect(back?.back).toEqual([]);
    expect(back?.forward?.map((p) => p.table)).toEqual(["orders"]);

    const fwd = stepTab(ws([back as WorkspaceTab]), "t", 1).tabs?.[0];
    expect(fwd?.table).toBe("orders");
    expect(fwd?.forward).toEqual([]);
  });

  it("drops the forward history when opening something new", () => {
    const a = navigateTab(ws([people]), "t", orders).tabs ?? [];
    const b = stepTab(ws(a), "t", -1).tabs ?? [];
    const c = navigateTab(ws(b), "t", { ...orders, title: "items", table: "items" }).tabs?.[0];
    expect(c?.forward).toEqual([]);
    expect(c?.back?.map((p) => p.table)).toEqual(["people"]);
  });

  it("does nothing without history", () => {
    expect(stepTab(ws([people]), "t", -1)).toEqual({});
  });
});
