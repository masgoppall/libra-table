// New rows from existing ones and from the clipboard: which columns keep their value.

import { describe, expect, it } from "vitest";
import { DEFAULT } from "@/lib/cells";
import { type ColumnInfo } from "@/types";
import {
  fieldValue,
  parseTsv,
  pastedFields,
  pastedRows,
  rememberCopy,
  rowFromCells,
  takesDefault,
} from "./rowCopies";

const col = (name: string, extra: Partial<ColumnInfo> = {}): ColumnInfo => ({
  name,
  dataType: "text",
  nullable: true,
  default: null,
  pkPosition: null,
  autoIncrement: false,
  comment: null,
  ...extra,
});

const COLUMNS = [
  col("id", { pkPosition: 1, autoIncrement: true, nullable: false }),
  col("name", { nullable: false }),
  col("note"),
  col("avatar", { dataType: "bytea" }),
];

describe("row copies", () => {
  it("leaves generated, auto-increment and defaulted keys to the database", () => {
    expect(takesDefault(COLUMNS[0] as ColumnInfo)).toBe(true);
    expect(takesDefault(col("uuid", { pkPosition: 1, default: "gen_random_uuid()" }))).toBe(true);
    expect(takesDefault(col("code", { pkPosition: 1 }))).toBe(false);
    expect(takesDefault(col("total", { extra: "STORED GENERATED" }))).toBe(true);
    expect(takesDefault(col("created_at", { extra: "DEFAULT_GENERATED" }))).toBe(false);
  });

  it("duplicates a row without its key, its pending defaults or binary previews", () => {
    const made = rowFromCells(COLUMNS, [7, "Alice", DEFAULT, { $binary: "ff", size: 1 }]);
    expect(made).toEqual({ values: { name: "Alice" }, skipped: ["avatar"] });
    expect(rowFromCells(COLUMNS, [8, "", null, null]).values).toEqual({
      name: "",
      note: null,
      avatar: null,
    });
  });

  it("reads tab-separated text and NULLs", () => {
    expect(parseTsv("a\tb\r\nc\td\n")).toEqual([
      ["a", "b"],
      ["c", "d"],
    ]);
    expect(fieldValue(COLUMNS[2], "NULL")).toBeNull();
    expect(fieldValue(COLUMNS[2], "")).toBeNull();
    expect(fieldValue(COLUMNS[1], "")).toBe("");
    expect(fieldValue(COLUMNS[1], "x")).toBe("x");
  });

  it("pastes rows copied from a grid by column name, keeping NULL apart from ''", () => {
    const text = "2\t\t";
    rememberCopy({ text, columns: ["id", "note", "name"], rows: [[2, null, ""]] });
    expect(pastedRows(COLUMNS, text)).toEqual({
      rows: [{ values: { note: null, name: "" }, skipped: [] }],
      matched: "name",
    });
    expect(pastedFields(text)).toEqual([["2", "NULL", ""]]);
  });

  it("pastes plain text only when it has a field per column", () => {
    expect(pastedRows(COLUMNS, "1\tBob\t\t")).toEqual({
      rows: [{ values: { name: "Bob", note: null, avatar: null }, skipped: [] }],
      matched: "position",
    });
    expect(pastedRows(COLUMNS, "Bob\tnote")).toBeNull();
  });
});
