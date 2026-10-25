import { describe, expect, it } from "vitest";
import { formatSql } from "./format";

describe("beautify", () => {
  it("formats per dialect and keeps variables, casts and placeholders", () => {
    expect(formatSql("select a, b::text from t where id = :id and x = $1", "postgres")).toBe(
      "SELECT\n  a,\n  b::text\nFROM\n  t\nWHERE\n  id = :id\n  AND x = $1",
    );
    expect(formatSql("select `a` from t where b = ? limit 1", "mysql")).toBe(
      "SELECT\n  `a`\nFROM\n  t\nWHERE\n  b = ?\nLIMIT\n  1",
    );
  });

  it("throws on text it can't parse", () => {
    expect(() => formatSql("select 'unterminated", "sqlite")).toThrow();
  });
});
