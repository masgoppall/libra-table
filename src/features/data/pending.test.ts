import { describe, expect, it } from "vitest";
import { DEFAULT } from "@/lib/cells";
import { type PageResult } from "@/types";
import * as p from "./pending";

const page: PageResult = {
  columns: [
    {
      name: "id",
      dataType: "int",
      nullable: false,
      default: null,
      pkPosition: 1,
      autoIncrement: true,
      comment: null,
    },
    {
      name: "name",
      dataType: "text",
      nullable: true,
      default: null,
      pkPosition: null,
      autoIncrement: false,
      comment: null,
    },
  ],
  rows: [
    [1, "Alice"],
    [2, "Bob"],
  ],
  primaryKey: ["id"],
  page: 0,
  pageSize: 100,
  hasMore: false,
  sql: "",
  elapsedMs: 0,
};

describe("pending edits", () => {
  it("records an edit and drops it when set back", () => {
    let e = p.editCell(p.emptyPending(), page, 1, 1, "Robert");
    expect(p.cellAt(e, page, 1, 1)).toBe("Robert");
    expect(p.isEdited(e, page, 1, 1)).toBe(true);
    e = p.editCell(e, page, 1, 1, "Bob");
    expect(p.pendingCount(e)).toBe(0);
  });

  it("new rows start with the column defaults", () => {
    const e = p.addInsert(p.emptyPending());
    expect(p.cellAt(e, page, 2, 0)).toEqual(DEFAULT);
  });

  it("deleting a new row removes it; deleting a loaded row toggles its mark", () => {
    let e = p.addInsert(p.emptyPending());
    e = p.toggleDeletes(e, new Set([0, 2]), 2);
    expect(e.inserts).toHaveLength(0);
    expect([...e.deletes]).toEqual([0]);
    e = p.toggleDeletes(e, new Set([0]), 2);
    expect(e.deletes.size).toBe(0);
  });

  it("builds changes keyed by primary key, skipping updates of deleted rows", () => {
    let e = p.editCell(p.emptyPending(), page, 0, 1, "Alicia");
    e = p.editCell(e, page, 1, 1, "Robert");
    e = p.toggleDeletes(e, new Set([1]), 2);
    e = p.editCell(p.addInsert(e), page, 2, 1, "Carol");
    expect(p.buildChanges(e, page)).toEqual([
      { kind: "update", pk: { id: 1 }, values: { name: "Alicia" } },
      { kind: "insert", values: { name: "Carol" } },
      { kind: "delete", pk: { id: 2 } },
    ]);
  });
});
