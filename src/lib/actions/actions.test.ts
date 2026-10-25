// @vitest-environment jsdom

import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { dispatch, LAYER, resetActions, useActions } from "./dispatch";
import { keyLabel } from "./keys";

describe("keyLabel", () => {
  it("formats keys for the platform", () => {
    expect(["⇧⌘K", "Ctrl+Shift+K"]).toContain(keyLabel("Mod+Shift+K"));
    expect(["⌘=", "Ctrl+="]).toContain(keyLabel("Mod+="));
  });
});

describe("dispatch", () => {
  afterEach(resetActions);

  it("returns false when no layer handles the action", () => {
    expect(dispatch("save")).toBe(false);
  });

  it("asks the highest layer first and falls through on false", () => {
    const order: string[] = [];
    renderHook(() => useActions(LAYER.app, true, { save: () => void order.push("app") }));
    const tab = renderHook(() =>
      useActions(LAYER.tab, true, {
        save: () => {
          order.push("tab");
          return false;
        },
      }),
    );
    expect(dispatch("save")).toBe(true);
    expect(order).toEqual(["tab", "app"]);
    tab.unmount();
    expect(dispatch("save")).toBe(true);
    expect(order).toEqual(["tab", "app", "app"]);
  });

  it("runs a key press and the same menu item once", () => {
    let runs = 0;
    renderHook(() => useActions(LAYER.app, true, { save: () => void runs++ }));
    dispatch("save", "key");
    dispatch("save", "menu");
    expect(runs).toBe(1);
  });

  it("ignores inactive layers", () => {
    let runs = 0;
    renderHook(() => useActions(LAYER.tab, false, { save: () => void runs++ }));
    expect(dispatch("save")).toBe(false);
    expect(runs).toBe(0);
  });
});
