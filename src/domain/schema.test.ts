import { describe, expect, it } from "vitest";
import { defaultSchema, fallbackSchema } from "./schema";

describe("schemas", () => {
  it("falls back to the usual default schema", () => {
    expect(fallbackSchema(["a", "public"])).toBe("public");
    expect(fallbackSchema(["INFORMATION_SCHEMA", "PUBLIC"])).toBe("PUBLIC");
    expect(fallbackSchema(["libratable_ds", "other"])).toBe("libratable_ds");
    expect(fallbackSchema([])).toBeUndefined();
  });

  it("uses the database as the schema for MySQL-style drivers", () => {
    expect(defaultSchema("mysql", "shop")).toBe("shop");
    expect(defaultSchema("postgres", "shop")).toBe("public");
  });
});
