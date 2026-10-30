import { describe, expect, it } from "vitest";
import { templateScript } from "./scripts";

const cols = [
  { name: "id", pkPosition: 1 },
  { name: "name", pkPosition: null },
];

describe("copy script as", () => {
  it("writes templates keyed by the primary key", () => {
    expect(templateScript("insert", "postgres", "public", "people", cols)).toBe(
      'INSERT INTO "public"."people" ("id", "name")\nVALUES (?, ?);',
    );
    expect(templateScript("update", "mysql", "shop", "people", cols)).toBe(
      "UPDATE `shop`.`people`\nSET `name` = ?\nWHERE `id` = ?;",
    );
    expect(templateScript("delete", "postgres", "public", "people", cols)).toBe(
      'DELETE FROM "public"."people"\nWHERE "id" = ?;',
    );
    expect(templateScript("select", "postgres", "public", "people", cols)).toBe(
      'SELECT "id", "name"\nFROM "public"."people";',
    );
  });
});
