import { describe, expect, it } from "vitest";
import { BUILT_IN_THEMES, themeCss, themeFromFile } from "./themes";

describe("color themes", () => {
  it("turns a theme into CSS variables over its base", () => {
    const css = themeCss(BUILT_IN_THEMES[0]!);
    expect(css).toContain("--st-row1: #102328;");
    expect(css).toContain("--accent: #2bb3a3;");
    expect(css).toContain("--accent-weak: color-mix(");
    expect(css.startsWith(":root:root:root {")).toBe(true);
  });

  it("reads a theme file in TablePlus' format", () => {
    const file = JSON.stringify({
      name: "Mine",
      base: "light",
      colors: { AppContentBackground: "#FAFAFA", GridCellSelectionBorder: "#0055ff", Other: "x" },
    });
    expect(themeFromFile(file, "t1")).toEqual({
      id: "t1",
      name: "Mine",
      base: "light",
      colors: { content: "#fafafa", accent: "#0055ff", selection: "#0055ff" },
    });
    expect(() => themeFromFile("{}", "x")).toThrow(/no colors/);
    expect(() => themeFromFile('{"colors":{"A":"#000000"}}', "x")).toThrow(/LibraTable uses/);
  });

  it("ships themes with readable selections", () => {
    for (const t of BUILT_IN_THEMES) {
      expect(t.colors.selection, t.name).toMatch(/^#[0-9a-f]{6}$/);
      expect(t.colors.text, t.name).toBeTruthy();
    }
  });
});
