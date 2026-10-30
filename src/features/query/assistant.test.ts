import { describe, expect, it } from "vitest";
import { schemaSummary } from "./assistant";

describe("assistant schema summary", () => {
  it("lists tables and their columns, and says how many fit", () => {
    const c = { schema: "public", schemas: ["public"], tables: { b: ["x"], a: ["id", "name"] } };
    expect(schemaSummary(c)).toEqual({ text: "a: id, name\nb: x", sent: 2, total: 2 });
    expect(schemaSummary(c, 14)).toEqual({ text: "a: id, name", sent: 1, total: 2 });
    expect(schemaSummary(null)).toEqual({ text: "", sent: 0, total: 0 });
  });
});
