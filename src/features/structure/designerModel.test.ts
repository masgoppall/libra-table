import { describe, expect, it } from "vitest";
import { type TableStructure } from "@/types";
import * as m from "./designerModel";

const structure: TableStructure = {
  kind: "table",
  columns: [
    {
      name: "id",
      dataType: "integer",
      nullable: false,
      default: null,
      pkPosition: 1,
      autoIncrement: true,
      comment: null,
    },
    {
      name: "note",
      dataType: "text",
      nullable: true,
      default: null,
      pkPosition: null,
      autoIncrement: false,
      comment: null,
    },
  ],
  indexes: [{ name: "t_pkey", columns: ["id"], unique: true, primary: true, definition: "" }],
  foreignKeys: [],
  ddl: "",
};

function existing(): m.DesignerState {
  const rows = m.rowsFromStructure(structure);
  return { creating: false, schema: "public", name: "t", comment: "", ...rows };
}

describe("table designer model", () => {
  it("has no change until something is edited", () => {
    expect(m.buildChange(existing())).toBeNull();
  });

  it("turns edits into alter ops", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    s.cols = m.patchRow(s.cols, note.key, { dataType: "varchar(10)" });
    s.cols = [...s.cols, m.newColumnRow("postgres", false)];
    const added = s.cols[2] as m.ColRow;
    s.cols = m.patchRow(s.cols, added.key, { name: "extra" });
    const change = m.buildChange(s);
    expect(change?.kind).toBe("alter_table");
    const ops = change?.kind === "alter_table" ? change.ops.map((o) => o.op) : [];
    expect(ops).toEqual(["modify_column", "add_column"]);
  });

  it("sets the primary key only when its columns change", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    s.cols = m.patchRow(s.cols, note.key, { primaryKey: true });
    const change = m.buildChange(s);
    const ops = change?.kind === "alter_table" ? change.ops : [];
    expect(ops).toContainEqual({ op: "set_primary_key", columns: ["id", "note"] });
  });

  it("drops existing rows and removes new ones", () => {
    const s = existing();
    const note = s.cols[1] as m.ColRow;
    expect(m.toggleDrop(s.cols, note.key)[1]?.dropped).toBe(true);
    const fresh = m.newIndexRow();
    expect(m.toggleDrop([fresh], fresh.key)).toEqual([]);
  });

  it("auto increment makes a column NOT NULL without a default", () => {
    const c = m.editColumn(
      { ...m.newColumnRow("postgres", false).cur, default: "0" },
      {
        autoIncrement: true,
      },
    );
    expect(c).toMatchObject({ nullable: false, default: null, autoIncrement: true });
  });

  it("validates a new table", () => {
    const s: m.DesignerState = {
      creating: true,
      schema: "public",
      name: "",
      comment: "",
      cols: [m.newColumnRow("postgres", true), m.newColumnRow("postgres", false)],
      idx: [m.newIndexRow()],
      fks: [],
    };
    expect(m.validate(s)).toEqual([
      "Name the table.",
      "Every column needs a name.",
      "Pick the columns of the new index.",
    ]);
  });
});
