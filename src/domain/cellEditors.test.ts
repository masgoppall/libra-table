import { describe, expect, it } from "vitest";
import { editorFor } from "./cellEditors";

describe("cell editors", () => {
  it("picks the editor from the column", () => {
    expect(editorFor("postgres", { dataType: "mood" }, ["sad", "ok"])).toEqual({
      kind: "choice",
      options: ["sad", "ok"],
    });
    expect(editorFor("postgres", { dataType: "boolean" }, undefined)).toEqual({
      kind: "choice",
      options: ["true", "false"],
    });
    expect(editorFor("mysql", { dataType: "tinyint(1)" }, undefined).kind).toBe("choice");
    expect(editorFor("sqlite", { dataType: "DATE" }, undefined)).toEqual({ kind: "date" });
    expect(editorFor("postgres", { dataType: "text" }, undefined)).toEqual({ kind: "text" });
  });
});
