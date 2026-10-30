// When a scheduled backup runs next, and what its files are called. Pure.

import { type BackupSchedule } from "@/types";

const HOUR = 3_600_000;

/** The first time on or after `from` that matches the schedule. */
export function nextSlot(s: BackupSchedule, from: number): number {
  const [h = 0, m = 0] = s.at.split(":").map(Number);
  const d = new Date(from);
  if (s.every === "hourly") {
    d.setMinutes(m, 0, 0);
    if (d.getTime() < from) d.setTime(d.getTime() + HOUR);
    return d.getTime();
  }
  d.setHours(h, m, 0, 0);
  if (d.getTime() < from) d.setDate(d.getDate() + 1);
  if (s.every === "weekly") {
    while (d.getDay() !== s.weekday) d.setDate(d.getDate() + 1);
  }
  return d.getTime();
}

/** Due when a slot has come since the last run (a new schedule starts counting when it
 * is made). A slot missed while the app was closed runs once, at the next check. */
export function isDue(s: BackupSchedule, now: number, created = now): boolean {
  if (!s.enabled) return false;
  const since = s.lastRun ?? created;
  return nextSlot(s, since + 1) <= now;
}

/** The file prefix of a database's scheduled backups (also what pruning matches). */
export function backupPrefix(database: string): string {
  const base =
    database
      .split("/")
      .pop()
      ?.replace(/\.[^.]*$/, "") || "backup";
  return base.replace(/[^\w.-]+/g, "_");
}

/** `shop-2026-10-30-0200.sql` (local time). */
export function scheduledFileName(database: string, ext: string, at: number): string {
  const d = new Date(at);
  const pad = (n: number) => String(n).padStart(2, "0");
  const day = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return `${backupPrefix(database)}-${day}-${pad(d.getHours())}${pad(d.getMinutes())}.${ext}`;
}

export function describeSchedule(s: BackupSchedule): string {
  const days = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
  if (s.every === "hourly") return `Every hour at :${s.at.slice(3)}`;
  if (s.every === "weekly") return `Every ${days[s.weekday]} at ${s.at}`;
  return `Every day at ${s.at}`;
}
