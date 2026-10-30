import { describe, expect, it } from "vitest";
import { findMatches, findPattern, nextMatch, replaceIn } from "./findReplace";

const opts = { query: "", matchCase: false, wholeWord: false, regex: false, column: null };
const grid = [
  ["Alice", "likes deals"],
  ["bob", null],
  ["Alice2", "a.b"],
];
const src = { rowCount: 3, visible: [0, 1], text: (r: number, c: number) => grid[r]?.[c] ?? null };

describe("search and replace within a table", () => {
  it("finds plain text, ignoring case unless asked", () => {
    const re = findPattern({ ...opts, query: "alice" });
    expect(re && findMatches(src, re, null)).toEqual([
      { row: 0, col: 0 },
      { row: 2, col: 0 },
    ]);
    const exact = findPattern({ ...opts, query: "alice", matchCase: true });
    expect(exact && findMatches(src, exact, null)).toEqual([]);
  });

  it("matches whole words and escapes plain text", () => {
    const word = findPattern({ ...opts, query: "Alice", wholeWord: true });
    expect(word && findMatches(src, word, 0)).toEqual([{ row: 0, col: 0 }]);
    const dot = findPattern({ ...opts, query: "." });
    expect(dot && findMatches(src, dot, null)).toEqual([{ row: 2, col: 1 }]);
  });

  it("replaces every match, with groups for regular expressions", () => {
    const re = findPattern({ ...opts, query: "(\\w+)ce", regex: true });
    expect(re && replaceIn("Alice", re, "$1x", true)).toBe("Alix");
    const plain = findPattern({ ...opts, query: "a" });
    expect(plain && replaceIn("Banana", plain, "$1", false)).toBe("B$1n$1n$1");
    expect(() => findPattern({ ...opts, query: "(", regex: true })).toThrow();
  });

  it("steps to the next match and wraps", () => {
    const m = [
      { row: 0, col: 1 },
      { row: 2, col: 0 },
    ];
    expect(nextMatch(m, { row: 0, col: 1 })).toBe(1);
    expect(nextMatch(m, { row: 2, col: 0 })).toBe(0);
    expect(nextMatch(m, { row: 1, col: 0 }, true)).toBe(0);
  });
});
