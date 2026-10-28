import { describe, expect, it } from "vitest";
import { newConnection } from "@/domain/connection";
import { type ConnectionView, type Group } from "@/types";
import { buildTree, groupPath, visibleConnections } from "./tree";

const conn = (id: string, name: string, groupId: string | null): ConnectionView => ({
  ...newConnection("postgres"),
  id,
  name,
  groupId,
  summary: "",
  hasDbPassword: false,
  hasSshPassword: false,
  hasSshKey: false,
  hasSshKeyPassphrase: false,
});

const group = (id: string, name: string, parentId: string | null, collapsed = false): Group => ({
  id,
  name,
  parentId,
  collapsed,
});

const groups = [
  group("local", "Local", null),
  group("acme", "Acme", null),
  group("stg", "Staging", "acme", true),
  group("prod", "Production", "acme"),
];
const conns = [
  conn("1", "Zeta", "local"),
  conn("2", "Alpha", "local"),
  conn("3", "Shop", "prod"),
  conn("4", "Old", "stg"),
  conn("5", "Reports", "acme"),
  conn("6", "Loose", null),
];

const labels = (rows: ReturnType<typeof buildTree>) =>
  rows.map((r) => `${"  ".repeat(r.depth)}${r.kind === "group" ? r.group.name : r.conn.name}`);

describe("connection tree", () => {
  it("nests groups, lists subgroups before connections and keeps the manual order", () => {
    expect(labels(buildTree(conns, groups, "", "manual"))).toEqual([
      "Local",
      "  Zeta",
      "  Alpha",
      "Acme",
      "  Staging",
      "  Production",
      "    Shop",
      "  Reports",
      "Loose",
    ]);
  });

  it("sorts by name within each group", () => {
    const rows = buildTree(conns, groups, "", "name");
    expect(labels(rows).slice(0, 3)).toEqual(["Local", "  Alpha", "  Zeta"]);
  });

  it("shows matches with their groups, even inside collapsed ones", () => {
    expect(labels(buildTree(conns, groups, "old", "manual"))).toEqual([
      "Acme",
      "  Staging",
      "    Old",
    ]);
    const rows = buildTree(conns, groups, "", "manual");
    expect(visibleConnections(rows).map((c) => c.name)).not.toContain("Old");
  });

  it("names a group with its parents", () => {
    expect(groupPath(groups[3] as Group, groups)).toBe("Acme / Production");
  });
});
