import { describe, expect, it } from "vitest";
import { writableExtra } from "@/domain/ddl";
import { type AlterOp, type ColumnInfo, type Dialect, type TableStructure } from "@/types";
import * as c from "./designerChange";
import * as m from "./designerModel";

const column = (name: string, extra: Partial<ColumnInfo> = {}): ColumnInfo => ({
  name,
  dataType: "text",
  nullable: true,
  default: null,
  pkPosition: null,
  autoIncrement: false,
  comment: null,
  ...extra,
});

const structure: TableStructure = {
  kind: "table",
  columns: [
    column("id", { dataType: "integer", nullable: false, pkPosition: 1, autoIncrement: true }),
    column("note"),
  ],
  indexes: [{ name: "t_pkey", columns: ["id"], unique: true, primary: true, definition: "" }],
  foreignKeys: [],
  ddl: "",
};

function existing(s = structure, driver: Dialect = "postgres"): m.DesignerState {
  const rows = m.rowsFromStructure(s);
  return { creating: false, driver, schema: "public", table: "t", name: "t", comment: "", ...rows };
}

const ops = (s: m.DesignerState): AlterOp[] => {
  const change = c.buildChange(s);
  return change?.kind === "alter_table" ? change.ops : [];
};

describe("table designer model", () => {
  it("has no change until something is edited", () => {
    expect(c.buildChange(existing())).toBeNull();
  });

  it("turns edits into alter ops", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    s.cols = m.patchRow(s.cols, note.key, { dataType: "varchar(10)" });
    s.cols = [...s.cols, m.newColumnRow("postgres", false)];
    const added = s.cols[2] as m.ColRow;
    s.cols = m.patchRow(s.cols, added.key, { name: "extra" });
    expect(ops(s).map((o) => o.op)).toEqual(["modify_column", "add_column"]);
  });

  it("sets the primary key only when its columns change", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    s.cols = m.patchRow(s.cols, note.key, { primaryKey: true });
    expect(ops(s)).toContainEqual({ op: "set_primary_key", columns: ["id", "note"] });
  });

  it("drops existing rows and removes new ones", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    expect(m.toggleDrop(s.cols, note.key)[1]?.dropped).toBe(true);
    const fresh = m.newIndexRow();
    expect(m.toggleDrop([fresh], fresh.key)).toEqual([]);
  });

  it("auto increment makes a column NOT NULL without a default", () => {
    const col = m.editColumn(
      { ...m.newColumnRow("postgres", false).cur, default: "0" },
      { autoIncrement: true },
    );
    expect(col).toMatchObject({ nullable: false, default: null, autoIncrement: true });
  });

  it("validates a new table", () => {
    const s: m.DesignerState = {
      creating: true,
      driver: "postgres",
      schema: "public",
      table: "",
      name: "",
      comment: "",
      cols: [m.newColumnRow("postgres", true), m.newColumnRow("postgres", false)],
      idx: [m.newIndexRow()],
      fks: [],
    };
    expect(c.validate(s)).toEqual([
      "Name the table.",
      "Every column needs a name.",
      "Pick the columns of the new index.",
    ]);
  });

  it("renames the table from the Name field, after the column changes", () => {
    const s = existing();
    s.name = " notes ";
    const change = c.buildChange(s);
    expect(change).toMatchObject({ kind: "alter_table", name: "t", ops: [], renameTo: "notes" });
    expect(c.pendingCount(change)).toBe(1);
  });
});

describe("MySQL columns", () => {
  const mysql: TableStructure = {
    ...structure,
    columns: ["a", "b", "c", "d"].map((n) =>
      column(n, { charset: "latin1", collation: "latin1_swedish_ci" }),
    ),
  };

  it("moves only the column that was dragged", () => {
    const s = existing(mysql, "mysql");
    const d = s.cols[3] as m.ColRow;
    s.cols = m.moveRow(s.cols, d.key, 1);
    expect(s.cols.map((r) => r.cur.name)).toEqual(["a", "d", "b", "c"]);
    expect(ops(s)).toEqual([
      expect.objectContaining({ op: "modify_column", position: { after: "a" } }),
    ]);
    // Dragged down instead: still one statement.
    const t = existing(mysql, "mysql");
    t.cols = m.moveRow(t.cols, (t.cols[0] as m.ColRow).key, 3);
    expect(t.cols.map((r) => r.cur.name)).toEqual(["b", "c", "a", "d"]);
    expect(ops(t)).toEqual([
      expect.objectContaining({ op: "modify_column", position: { after: "c" } }),
    ]);
  });

  it("places added columns, and leaves appended ones at the end", () => {
    const s = existing(mysql, "mysql");
    const first = m.newColumnRow("mysql", false);
    const last = m.newColumnRow("mysql", false);
    s.cols = [first, ...s.cols, last];
    s.cols = m.patchRow(m.patchRow(s.cols, first.key, { name: "x" }), last.key, { name: "z" });
    const added = ops(s).filter((o) => o.op === "add_column");
    expect(added).toEqual([
      expect.objectContaining({ position: "first" }),
      expect.not.objectContaining({ position: expect.anything() }),
    ]);
  });

  it("never sends positions to drivers that can't move columns", () => {
    const s = existing(mysql, "postgres");
    s.cols = m.moveRow(s.cols, (s.cols[3] as m.ColRow).key, 0);
    expect(ops(s).every((o) => !("position" in o))).toBe(true);
  });

  it("keeps character sets on text types only, and collations with their set", () => {
    const base = m.rowsFromStructure(mysql).cols[0]?.cur;
    if (!base) throw new Error("no column");
    expect(m.editColumn(base, { dataType: "int" })).toMatchObject({
      charset: null,
      collation: null,
    });
    expect(m.editColumn(base, { charset: "utf8mb4" }).collation).toBeNull();
    expect(m.editColumn(base, { charset: "latin1" }).collation).toBe("latin1_swedish_ci");
  });

  it("reads EXTRA without the parts that aren't written back", () => {
    expect(writableExtra("DEFAULT_GENERATED on update CURRENT_TIMESTAMP")).toBe(
      "on update CURRENT_TIMESTAMP",
    );
    expect(writableExtra("auto_increment")).toBeNull();
    expect(writableExtra("")).toBeNull();
  });
});
