import { describe, expect, it } from "vitest";
import { jumpTargets, type JumpSource } from "./jump";

const values = [
  ["1", "Alice", "likes deals"],
  ["2", "Bob", ""],
];
const src: JumpSource = {
  columns: ["id", "name", "note"],
  visible: [0, 1, 2],
  rowCount: 2,
  rowOffset: 300,
  text: (r, c) => values[r]?.[c] ?? "",
};

describe("jump to column or row", () => {
  it("lists the columns by name, best match first", () => {
    const kinds = jumpTargets(src, "na").map((t) => (t.kind === "column" ? t.name : t.kind));
    expect(kinds[0]).toBe("name");
    expect(jumpTargets(src, "").length).toBe(3);
  });

  it("finds a row by the number shown on this page", () => {
    expect(jumpTargets(src, "302")[0]).toEqual({ kind: "row", row: 1, label: 302 });
    expect(jumpTargets(src, "5").some((t) => t.kind === "row")).toBe(false);
  });

  it("searches only the values after >", () => {
    expect(jumpTargets(src, "> bob")).toEqual([
      { kind: "value", row: 1, col: 1, column: "name", text: "Bob" },
    ]);
  });
});
