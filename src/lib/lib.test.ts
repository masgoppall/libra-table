import { describe, expect, it } from "vitest";
import { displayCell, isNumericType, prettyJson, sameValue } from "./cells";
import { copyRows } from "./copyRows";
import { plural } from "./format";
import { containsMatch, fuzzyMatch, search } from "./fuzzy";
import { qualifiedName, selectAllSql } from "./sql";

describe("cells", () => {
  it("displays cells", () => {
    expect(displayCell(null)).toBe("NULL");
    expect(displayCell({ $binary: "00ff", size: 2048 })).toBe("<binary 2.0 KB>");
    expect(displayCell("a\nb")).toBe("a ⏎ b");
    expect(displayCell(42)).toBe("42");
  });

  it("detects numeric types", () => {
    expect(isNumericType("integer")).toBe(true);
    expect(isNumericType("numeric(10,2)")).toBe(true);
    expect(isNumericType("BIGINT UNSIGNED")).toBe(true);
    expect(isNumericType("interval")).toBe(false);
    expect(isNumericType("text")).toBe(false);
  });

  it("pretty prints JSON only when valid", () => {
    expect(prettyJson('{"a":1}')).toBe('{\n  "a": 1\n}');
    expect(prettyJson("{not json")).toBeNull();
    expect(prettyJson("plain")).toBeNull();
  });

  it("compares edits as the text the user sees", () => {
    expect(sameValue(42, "42")).toBe(true);
    expect(sameValue(null, "")).toBe(false);
    expect(sameValue("a", null)).toBe(false);
  });
});

describe("copyRows", () => {
  const cols = ["id", "name"];
  const rows = [
    [1, "O'Brien"],
    [2, null],
  ];

  it("copies rows in every format", () => {
    expect(copyRows("tsv", cols, rows, { driver: "postgres", header: true })).toBe(
      "id\tname\n1\tO'Brien\n2\t",
    );
    expect(copyRows("csv", cols, [[1, 'a,"b"']], { driver: "postgres" })).toBe('1,"a,""b"""');
    expect(JSON.parse(copyRows("json", cols, rows, { driver: "postgres" }))).toEqual([
      { id: 1, name: "O'Brien" },
      { id: 2, name: null },
    ]);
    expect(copyRows("sql", cols, rows, { driver: "mysql", table: "shop.people" })).toBe(
      "INSERT INTO `shop`.`people` (`id`, `name`) VALUES (1, 'O''Brien');\n" +
        "INSERT INTO `shop`.`people` (`id`, `name`) VALUES (2, NULL);",
    );
    expect(copyRows("markdown", cols, [[1, "a|b"]], { driver: "sqlite" })).toBe(
      "| id | name |\n| --- | --- |\n| 1 | a\\|b |",
    );
  });
});

describe("sql", () => {
  it("quotes identifiers per driver", () => {
    expect(qualifiedName("postgres", "public", 'we"ird')).toBe('"public"."we""ird"');
    expect(selectAllSql("mysql", "shop", "orders")).toBe(
      "SELECT * FROM `shop`.`orders` LIMIT 100;",
    );
  });
});

describe("format", () => {
  it("pluralizes", () => {
    expect(plural(1, "row")).toBe("1 row");
    expect(plural(1200, "row")).toBe("1,200 rows");
    expect(plural(2, "entry", "entries")).toBe("2 entries");
  });
});

describe("fuzzy search", () => {
  it("matches subsequences and prefers word starts", () => {
    expect(fuzzyMatch("usr", "users")).not.toBeNull();
    expect(fuzzyMatch("xyz", "users")).toBeNull();
    const a = fuzzyMatch("oi", "order_items");
    const b = fuzzyMatch("oi", "notification");
    expect(a && b && a.score > b.score).toBe(true);
  });

  it("contains mode is a plain substring match", () => {
    expect(containsMatch("item", "order_items")?.indexes).toEqual([6, 7, 8, 9]);
    expect(containsMatch("oi", "order_items")).toBeNull();
  });

  it("orders results by score and keeps order for empty query", () => {
    const items = ["notification", "order_items", "orders"];
    expect(search(items, "", "fuzzy", (s) => s).map((r) => r.item)).toEqual(items);
    expect(search(items, "ord", "fuzzy", (s) => s)[0]?.item).toMatch(/^order/);
  });
});
