import { describe, expect, it } from "vitest";
import { type BackupSchedule } from "@/types";
import {
  backupPrefix,
  describeSchedule,
  isDue,
  nextSlot,
  scheduledFileName,
} from "./backupSchedule";

const base: BackupSchedule = {
  id: "b",
  name: "",
  connectionId: "c",
  databases: ["shop"],
  folder: "/b",
  every: "daily",
  at: "02:00",
  weekday: 1,
  keep: 7,
  enabled: true,
  lastRun: null,
  lastError: null,
};
const at = (s: string) => new Date(s).getTime();

describe("backup schedules", () => {
  it("finds the next slot", () => {
    expect(nextSlot(base, at("2026-10-30T01:00:00"))).toBe(at("2026-10-30T02:00:00"));
    expect(nextSlot(base, at("2026-10-30T03:00:00"))).toBe(at("2026-10-31T02:00:00"));
    const weekly = { ...base, every: "weekly" as const, weekday: 1 };
    // 2026-10-30 is a Friday; the next Monday is 11-02.
    expect(nextSlot(weekly, at("2026-10-30T03:00:00"))).toBe(at("2026-11-02T02:00:00"));
    const hourly = { ...base, every: "hourly" as const, at: "00:15" };
    expect(nextSlot(hourly, at("2026-10-30T03:20:00"))).toBe(at("2026-10-30T04:15:00"));
  });

  it("is due once a slot has passed since the last run", () => {
    const ran = { ...base, lastRun: at("2026-10-30T02:00:30") };
    expect(isDue(ran, at("2026-10-30T23:00:00"))).toBe(false);
    expect(isDue(ran, at("2026-10-31T02:00:00"))).toBe(true);
    expect(isDue({ ...ran, enabled: false }, at("2026-11-05T02:00:00"))).toBe(false);
  });

  it("names the files", () => {
    expect(backupPrefix("/data/My App.db")).toBe("My_App");
    expect(scheduledFileName("shop", "sql", at("2026-10-30T02:05:00"))).toBe(
      "shop-2026-10-30-0205.sql",
    );
    expect(describeSchedule({ ...base, every: "weekly" })).toBe("Every Monday at 02:00");
  });
});
