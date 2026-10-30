// Running scheduled backups while LibraTable is open: connect if needed, back up each
// database to a dated file, keep the newest N, and close the connection again if it
// wasn't open on screen.

import { listen } from "@tauri-apps/api/event";
import { api } from "@/api";
import { backupPrefix, isDue, scheduledFileName } from "@/domain/backupSchedule";
import { uid } from "@/lib/uid";
import { store } from "@/store";
import { type AppError, type BackupSchedule, type ToolProgress } from "@/types";
import { toastInfo, toastSuccess } from "@/services/errors";
import { backupExtension, defaultBackup } from "./backupModel";

const message = (e: unknown) => (e as AppError)?.message ?? String(e);

/** Starts a backup job and waits for its last "tool-progress" event. */
async function backupJob(id: string, db: string | null, path: string): Promise<void> {
  const jobId = uid();
  let settle: (err: AppError | null) => void = () => undefined;
  const done = new Promise<AppError | null>((resolve) => (settle = resolve));
  const unlisten = await listen<ToolProgress>("tool-progress", (e) => {
    if (e.payload.jobId === jobId && e.payload.done) settle(e.payload.error);
  });
  try {
    await api.startBackup(id, db, jobId, { ...defaultBackup(), path });
    const err = await done;
    if (err) throw err;
  } finally {
    unlisten();
  }
}

/** Runs one schedule now; resolves to the error message, or null when it worked. */
export async function runSchedule(s: BackupSchedule, now = Date.now()): Promise<string | null> {
  if (!store.connection(s.connectionId)) return "The connection no longer exists.";
  const wasOpen = !!store.get().workspaces[s.connectionId];
  try {
    const info = await api.connect(s.connectionId);
    const ext = backupExtension(info.dialect, "plain");
    const databases = s.databases.length ? s.databases : [info.database];
    for (const database of databases) {
      const db = database === info.database ? null : database;
      const folder = s.folder.replace(/\/+$/, "");
      await backupJob(s.connectionId, db, `${folder}/${scheduledFileName(database, ext, now)}`);
      if (s.keep > 0) await api.pruneBackups(folder, backupPrefix(database), s.keep);
    }
    return null;
  } catch (e) {
    return message(e);
  } finally {
    if (!wasOpen && !store.get().workspaces[s.connectionId]) {
      await api.disconnect(s.connectionId).catch(() => undefined);
    }
  }
}

/** Saves how a run went into the schedule (the settings may have changed meanwhile). */
async function record(id: string, lastRun: number, lastError: string | null) {
  const settings = store.get().settings;
  if (!settings) return;
  const backupSchedules = settings.backupSchedules.map((b) =>
    b.id === id ? { ...b, lastRun, lastError } : b,
  );
  await store.get().saveSettings({ backupSchedules });
}

let running = false;

/** Runs the schedules that are due, one after another. */
export async function runDueSchedules(now = Date.now()) {
  if (running) return;
  running = true;
  try {
    const due = (store.get().settings?.backupSchedules ?? []).filter((s) => isDue(s, now));
    for (const s of due) await runAndRecord(s, now);
  } finally {
    running = false;
  }
}

export async function runAndRecord(s: BackupSchedule, now = Date.now()) {
  const label = s.name || "Scheduled backup";
  const error = await runSchedule(s, now);
  await record(s.id, now, error).catch(() => undefined);
  if (error) toastInfo(`${label} failed.`, error);
  else toastSuccess(`${label} done.`, s.folder);
}
