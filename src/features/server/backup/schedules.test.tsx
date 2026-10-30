// @vitest-environment jsdom
// Backup schedules: a due schedule connects, backs up each database to a dated file,
// prunes old ones and disconnects again; the dialog saves schedules into the settings.

import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp, setupApp } from "@/test/app";
import { store } from "@/store";
import { type BackupSchedule } from "@/types";
import { runSchedule } from "./backupRunner";

const h = setupApp();

const schedule: BackupSchedule = {
  id: "b1",
  name: "Nightly",
  connectionId: "c-local",
  databases: ["app", "analytics"],
  folder: "/backups/",
  every: "daily",
  at: "02:00",
  weekday: 1,
  keep: 3,
  enabled: true,
  lastRun: null,
  lastError: null,
};

/** Answers each backup job as soon as it starts. */
async function finishJobs(count: number) {
  for (let i = 1; i <= count; i++) {
    await waitFor(() => expect(h.backend.callsOf("start_backup").length).toBe(i));
    const call = h.backend.callsOf("start_backup")[i - 1]?.args;
    const base = { jobId: call?.jobId, line: null, bytes: 1, total: null, error: null, path: "" };
    await act(() => emit("tool-progress", { ...base, done: true }));
  }
}

describe("backup schedules", () => {
  it("backs up every database to a dated file and keeps the newest", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    const at = new Date("2026-10-30T02:00:00").getTime();
    const run = runSchedule(schedule, at);
    await finishJobs(2);
    expect(await run).toBeNull();
    const calls = h.backend
      .callsOf("start_backup")
      .map((c) => [c.args.database, c.args.request.path]);
    expect(calls).toEqual([
      [null, "/backups/app-2026-10-30-0200.sql"],
      ["analytics", "/backups/analytics-2026-10-30-0200.sql"],
    ]);
    expect(h.backend.callsOf("prune_backups").map((c) => c.args)).toEqual([
      { folder: "/backups", prefix: "app", keep: 3 },
      { folder: "/backups", prefix: "analytics", keep: 3 },
    ]);
    // It wasn't open on screen: the connection is let go again.
    expect(h.backend.callsOf("disconnect").length).toBe(1);
    expect(store.get().workspaces["c-local"]).toBeUndefined();
  });

  it("adds a schedule in the dialog", async () => {
    renderApp();
    await screen.findByText("Local Postgres");
    act(() => store.get().setBackupSchedulesOpen(true));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByText("New schedule"));
    fireEvent.change(within(dialog).getByLabelText("Databases"), {
      target: { value: "app, analytics" },
    });
    fireEvent.change(within(dialog).getByLabelText("How often"), { target: { value: "weekly" } });
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Save" })));
    const saved = h.backend.lastCall("save_settings")?.args.settings.backupSchedules;
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({
      connectionId: "c-local",
      databases: ["app", "analytics"],
      every: "weekly",
    });
  });
});
