// Pure rules of the server tools: process filtering and wording, role forms, grant
// targets, backup names and progress.

import { describe, expect, it } from "vitest";
import { PROCESSES } from "@/test/fakeServer";
import { type PgRole } from "@/types";
import {
  appendLine,
  backupExtension,
  backupFileName,
  progressText,
  toggleName,
} from "./backup/backupModel";
import { filterProcesses, formatAge, stopWording } from "./processes/processModel";
import { buildTarget, parseConnLimit, roleToSpec, togglePrivilege } from "./users/userModel";

describe("process list", () => {
  it("filters by text and hides idle rows", () => {
    expect(filterProcesses(PROCESSES, "", false)).toHaveLength(2);
    expect(filterProcesses(PROCESSES, "", true).map((p) => p.id)).toEqual([101]);
    expect(filterProcesses(PROCESSES, "WORKER", false).map((p) => p.id)).toEqual([101]);
    expect(filterProcesses(PROCESSES, "102", false).map((p) => p.id)).toEqual([102]);
  });

  it("formats durations", () => {
    expect(formatAge(null)).toBe("—");
    expect(formatAge(850)).toBe("850 ms");
    expect(formatAge(12_400)).toBe("12 s");
    expect(formatAge(185_000)).toBe("3m 05s");
    expect(formatAge(2 * 3_600_000 + 600_000)).toBe("2h 10m");
    expect(formatAge(3 * 86_400_000 + 4 * 3_600_000)).toBe("3d 4h");
  });

  it("warns before stopping LibraTable's own connection", () => {
    const [other, own] = PROCESSES as [(typeof PROCESSES)[0], (typeof PROCESSES)[0]];
    const w = stopWording("postgres", other, "cancel");
    expect(w.typeToConfirm).toBeUndefined();
    expect(w.message).toContain("process 101 (app on app)");
    const k = stopWording("mysql", own, "kill");
    expect(k.message).toContain("connection 102");
    expect(k.message).toContain("LibraTable's own connection");
    expect(k.typeToConfirm).toBe("102");
  });
});

describe("users", () => {
  it("maps a role to the form and back", () => {
    const role: PgRole = {
      name: "app",
      login: true,
      superuser: false,
      createDb: true,
      createRole: false,
      replication: false,
      connLimit: -1,
      validUntil: "infinity",
      memberOf: ["readers"],
    };
    expect(roleToSpec(role)).toMatchObject({ connLimit: null, validUntil: null, createDb: true });
    expect(parseConnLimit("")).toBeNull();
    expect(parseConnLimit(" 5 ")).toBe(5);
    expect(parseConnLimit("-1")).toBeUndefined();
  });

  it("builds grant targets only when complete", () => {
    const f = { database: "shop", schema: "", table: "" };
    expect(buildTarget("global", f)).toEqual({ on: "global" });
    expect(buildTarget("database", f)).toEqual({ on: "database", name: "shop" });
    expect(buildTarget("table", { ...f, schema: "public" })).toBeNull();
    expect(buildTarget("table", { ...f, schema: "public", table: "t" })).toEqual({
      on: "table",
      schema: "public",
      name: "t",
    });
  });

  it("keeps ALL PRIVILEGES alone", () => {
    expect(togglePrivilege(["SELECT"], "INSERT", true)).toEqual(["SELECT", "INSERT"]);
    expect(togglePrivilege(["SELECT"], "ALL PRIVILEGES", true)).toEqual(["ALL PRIVILEGES"]);
    expect(togglePrivilege(["ALL PRIVILEGES"], "SELECT", true)).toEqual(["SELECT"]);
    expect(togglePrivilege(["SELECT", "INSERT"], "SELECT", false)).toEqual(["INSERT"]);
  });
});

describe("backup", () => {
  it("names files and reports progress", () => {
    const day = new Date("2026-10-25T12:00:00Z");
    expect(backupFileName("shop", "sql", day)).toBe("shop-2026-10-25.sql");
    expect(backupFileName("/data/app.db", "db", day)).toBe("app-2026-10-25.db");
    expect(backupExtension("postgres", "custom")).toBe("dump");
    expect(backupExtension("mysql", "custom")).toBe("sql");
    expect(backupExtension("sqlite", "plain")).toBe("db");
    expect(progressText(2048, null, "written")).toBe("2.0 KB written");
    expect(progressText(512, 2048, "read")).toBe("512 B of 2.0 KB (25%)");
    expect(appendLine(["a", "b"], "c", 2)).toEqual(["b", "c"]);
    expect(toggleName(["a"], "b", true)).toEqual(["a", "b"]);
    expect(toggleName(["a", "b"], "a", false)).toEqual(["b"]);
  });
});
