import { describe, expect, it } from "vitest";
import { type DiagramTable } from "@/types";
import {
  autoLayout,
  bounds,
  edgePath,
  fitView,
  layers,
  mergeLayout,
  rectOf,
  related,
  tableSize,
  zoomAt,
} from "./layout";

function table(name: string, refs: string[] = [], cols = 2): DiagramTable {
  return {
    name,
    columns: Array.from({ length: cols }, (_, i) => ({
      name: i === 0 ? "id" : `c${i}`,
      dataType: "integer",
      nullable: i > 0,
      primaryKey: i === 0,
    })),
    foreignKeys: refs.map((r) => ({
      name: `${name}_${r}`,
      columns: [`${r}_id`],
      refSchema: "public",
      refTable: r,
      refColumns: ["id"],
    })),
  };
}

describe("diagram layout", () => {
  it("layers tables by foreign key depth, surviving cycles", () => {
    const ts = [table("orders", ["users", "products"]), table("users"), table("products")];
    ts.push(table("items", ["orders"]), table("a", ["b"]), table("b", ["a"]));
    const l = layers(ts);
    expect([l.get("users"), l.get("products"), l.get("orders"), l.get("items")]).toEqual([
      0, 0, 1, 2,
    ]);
    expect(l.get("a")).toBeGreaterThanOrEqual(0);
    expect(l.get("b")).toBeGreaterThanOrEqual(0);
  });

  it("places referenced tables left of the tables that reference them", () => {
    const ts = [table("orders", ["users"]), table("users"), table("logs")];
    const pos = autoLayout(ts);
    expect(pos.users?.x).toBe(0);
    expect(pos.orders?.x).toBeGreaterThan(tableSize(table("users")).w);
    // Tables in one layer are stacked without overlapping.
    expect(pos.logs?.y).not.toBe(pos.users?.y);
  });

  it("falls back to a grid without foreign keys", () => {
    const ts = ["a", "b", "c", "d"].map((n) => table(n));
    const pos = autoLayout(ts);
    expect(pos.a).toEqual({ x: 0, y: 0 });
    expect(pos.b?.y).toBe(0);
    expect(pos.c?.x).toBe(0);
    expect(pos.c?.y).toBeGreaterThan(0);
  });

  it("keeps saved positions and puts new tables below them", () => {
    const ts = [table("a"), table("b")];
    const merged = mergeLayout(ts, { a: { x: 500, y: 40 } });
    expect(merged.a).toEqual({ x: 500, y: 40 });
    expect(merged.b?.y).toBeGreaterThan(40 + tableSize(table("a")).h);
  });

  it("sizes boxes by their rows and draws connectors between facing sides", () => {
    const t = table("users", [], 3);
    expect(tableSize(t).h).toBe(30 + 3 * 20 + 6);
    const from = rectOf(t, { users: { x: 0, y: 0 } });
    const to = rectOf(t, { users: { x: 400, y: 100 } });
    expect(edgePath(from, 1, to, 0)).toMatch(new RegExp(`^M ${from.w} 60 C`));
    expect(edgePath(to, 0, from, 1)).toMatch(/^M 400 140 C/);
    expect(bounds([from, to])).toEqual({ x: 0, y: 0, w: 400 + to.w, h: 100 + to.h });
  });

  it("fits and zooms around a point", () => {
    const v = fitView({ x: 0, y: 0, w: 2000, h: 1000 }, { w: 1064, h: 564 });
    expect(v.scale).toBeCloseTo(0.5);
    expect(v.x).toBeCloseTo(32);
    const z = zoomAt({ x: 0, y: 0, scale: 1 }, 2, { x: 100, y: 100 });
    expect(z).toEqual({ scale: 2, x: -100, y: -100 });
    expect(zoomAt(z, 100, { x: 0, y: 0 }).scale).toBe(2.5);
  });

  it("finds a table's relations both ways", () => {
    const ts = [table("orders", ["users"]), table("users"), table("logs")];
    expect([...related(ts, "users")].sort()).toEqual(["orders", "users"]);
  });
});
