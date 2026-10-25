import { describe, expect, it } from "vitest";
import { newConnection } from "./connection";
import { writeConfirmation } from "./safeMode";

describe("safe mode", () => {
  it("follows the tag in automatic mode", () => {
    const c = newConnection("postgres");
    expect(writeConfirmation(c)).toBe("none");
    expect(writeConfirmation({ ...c, tag: "production" })).toBe("alert");
  });

  it("uses the chosen level otherwise", () => {
    const c = newConnection("mysql");
    const at = (safeMode: typeof c.options.safeMode) =>
      writeConfirmation({ ...c, tag: "production", options: { ...c.options, safeMode } });
    expect(at("silent")).toBe("none");
    expect(at("alert_all")).toBe("alert");
    expect(at("safe_writes")).toBe("safe");
  });
});
