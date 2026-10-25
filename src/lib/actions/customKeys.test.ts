import { afterEach, describe, expect, it } from "vitest";
import { conflictsWith, keysFor, setCustomShortcuts, shortcutFor } from "./keys";

describe("custom shortcuts", () => {
  afterEach(() => setCustomShortcuts({}));

  it("replace the default keys of an action", () => {
    expect(keysFor("history")).toBe("Mod+Y");
    setCustomShortcuts({ history: "Mod+Shift+Y" });
    expect(keysFor("history")).toBe("Mod+Shift+Y");
    expect(shortcutFor("history")).toMatch(/Y$/);
  });

  it("report actions already bound to the same keys", () => {
    expect(conflictsWith("Mod+P", "history")).toEqual(["open-anything"]);
    expect(conflictsWith("Mod+Alt+J", "history")).toEqual([]);
  });
});
