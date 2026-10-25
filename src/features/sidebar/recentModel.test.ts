import { describe, expect, it } from "vitest";
import { type SidebarItem } from "@/types";
import {
  addRecent,
  isPinned,
  pinnedFirst,
  RECENT_LIMIT,
  recentIn,
  removeRecent,
  togglePin,
} from "./recentModel";

const item = (name: string, extra: Partial<SidebarItem> = {}): SidebarItem => ({
  database: "app",
  schema: "public",
  kind: "table",
  name,
  ...extra,
});

describe("recent items", () => {
  it("moves a reopened item to the front", () => {
    let list = [item("a"), item("b"), item("c")];
    list = addRecent(list, item("c", { openedAt: 5 }));
    expect(list.map((i) => i.name)).toEqual(["c", "a", "b"]);
    expect(list[0]?.openedAt).toBe(5);
  });

  it("keeps RECENT_LIMIT per database and leaves other databases alone", () => {
    let list: SidebarItem[] = [item("other", { database: "analytics" })];
    for (let i = 0; i < RECENT_LIMIT + 3; i++) list = addRecent(list, item(`t${i}`));
    expect(recentIn(list, "app")).toHaveLength(RECENT_LIMIT);
    expect(recentIn(list, "app")[0]?.name).toBe(`t${RECENT_LIMIT + 2}`);
    expect(recentIn(list, "analytics").map((i) => i.name)).toEqual(["other"]);
  });

  it("tells overloads and kinds apart", () => {
    const f1 = item("add", { kind: "function", arguments: "a integer" });
    const f2 = item("add", { kind: "function", arguments: "a text" });
    const list = addRecent(addRecent([], f1), f2);
    expect(list).toHaveLength(2);
    expect(removeRecent(list, f1)).toEqual([f2]);
  });
});

describe("pins", () => {
  it("toggles and floats pinned items to the top of their section", () => {
    let pinned = togglePin([], item("zeta", { openedAt: 9 }));
    pinned = togglePin(pinned, item("beta"));
    expect(pinned[0]).toEqual(item("zeta"));
    expect(isPinned(pinned, item("beta"))).toBe(true);
    const names = ["alpha", "beta", "gamma", "zeta"];
    const ordered = pinnedFirst(names, pinned, (n) => item(n));
    expect(ordered).toEqual(["zeta", "beta", "alpha", "gamma"]);
    pinned = togglePin(pinned, item("zeta"));
    expect(pinnedFirst(names, pinned, (n) => item(n))).toEqual(["beta", "alpha", "gamma", "zeta"]);
    // A pin in another schema doesn't apply here.
    const elsewhere = [item("gamma", { schema: "audit" })];
    expect(pinnedFirst(names, elsewhere, (n) => item(n))).toEqual(names);
  });
});
