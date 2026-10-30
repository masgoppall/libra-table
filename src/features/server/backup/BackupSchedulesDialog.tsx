// Backup schedules (TablePlus' "Backup schedules"): backups of saved connections that run
// on their own while LibraTable is open, into a folder, keeping the newest few.

import { open } from "@tauri-apps/plugin-dialog";
import { CalendarClock, FolderOpen, Play, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Check } from "@/components/Check";
import { Modal } from "@/components/Modal";
import { describeSchedule, nextSlot } from "@/domain/backupSchedule";
import { uid } from "@/lib/uid";
import { showError } from "@/services/errors";
import { store, useStore } from "@/store";
import { type BackupSchedule } from "@/types";
import { runAndRecord } from "./backupRunner";

const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const BACKUP_DRIVERS = new Set(["postgres", "mysql", "mariadb", "sqlite"]);

function newSchedule(connectionId: string): BackupSchedule {
  return {
    id: uid(),
    name: "",
    connectionId,
    databases: [],
    folder: "",
    every: "daily",
    at: "02:00",
    weekday: 1,
    keep: 7,
    enabled: true,
    // A new schedule counts from now: its first run is the next slot.
    lastRun: Date.now(),
    lastError: null,
  };
}

function Editor(p: { s: BackupSchedule; onChange: (patch: Partial<BackupSchedule>) => void }) {
  const { s, onChange } = p;
  const connections = useStore((st) => st.connections).filter((c) => BACKUP_DRIVERS.has(c.driver));
  const pickFolder = async () => {
    const dir = await open({ directory: true, title: "Folder for the backups" });
    if (typeof dir === "string") onChange({ folder: dir });
  };
  return (
    <div className="form-grid">
      <label>Name</label>
      <input
        className="input"
        value={s.name}
        placeholder="Nightly backup"
        onChange={(e) => onChange({ name: e.target.value })}
      />
      <label>Connection</label>
      <select
        className="select"
        aria-label="Connection to back up"
        value={s.connectionId}
        onChange={(e) => onChange({ connectionId: e.target.value })}
      >
        {connections.map((c) => (
          <option key={c.id} value={c.id}>
            {c.name}
          </option>
        ))}
      </select>
      <label>Databases</label>
      <input
        className="input"
        aria-label="Databases"
        value={s.databases.join(", ")}
        placeholder="Empty: the connection's database"
        onChange={(e) =>
          onChange({
            databases: e.target.value
              .split(",")
              .map((d) => d.trim())
              .filter(Boolean),
          })
        }
      />
      <label>Folder</label>
      <div className="row">
        <input
          className="input grow mono"
          readOnly
          value={s.folder}
          placeholder="Choose a folder"
        />
        <button className="btn sm" onClick={() => void pickFolder()}>
          <FolderOpen size={13} /> Choose…
        </button>
      </div>
      <label>Runs</label>
      <div className="row wrap">
        <select
          className="select"
          aria-label="How often"
          value={s.every}
          onChange={(e) => onChange({ every: e.target.value as BackupSchedule["every"] })}
        >
          <option value="hourly">Every hour</option>
          <option value="daily">Every day</option>
          <option value="weekly">Every week</option>
        </select>
        {s.every === "weekly" && (
          <select
            className="select"
            aria-label="Day"
            value={s.weekday}
            onChange={(e) => onChange({ weekday: Number(e.target.value) })}
          >
            {DAYS.map((d, i) => (
              <option key={d} value={i}>
                {d}
              </option>
            ))}
          </select>
        )}
        <input
          className="input"
          type="time"
          aria-label="At"
          value={s.at}
          onChange={(e) => onChange({ at: e.target.value || "02:00" })}
        />
      </div>
      <label>Keep</label>
      <div className="row">
        <input
          className="input num-input"
          type="number"
          min={0}
          max={999}
          aria-label="Backups to keep"
          value={s.keep}
          onChange={(e) => onChange({ keep: Math.max(0, Number(e.target.value) || 0) })}
        />
        <span className="muted">newest backups per database (0 keeps all)</span>
      </div>
      <span />
      <Check checked={s.enabled} onChange={(enabled) => onChange({ enabled })}>
        On
      </Check>
    </div>
  );
}

function status(s: BackupSchedule): string {
  if (!s.enabled) return "Off";
  const next = new Date(nextSlot(s, Math.max(Date.now(), (s.lastRun ?? 0) + 1)));
  const failed = s.lastError ? ` · last run failed: ${s.lastError}` : "";
  return `${describeSchedule(s)} · next ${next.toLocaleString()}${failed}`;
}

export function BackupSchedulesDialog({ onClose }: { onClose: () => void }) {
  const saved = useStore((st) => st.settings?.backupSchedules ?? []);
  const firstConnection = useStore((st) => st.connections[0]?.id ?? "");
  const [list, setList] = useState<BackupSchedule[]>(saved);
  const [current, setCurrent] = useState<string | null>(saved[0]?.id ?? null);
  const selected = list.find((s) => s.id === current);

  const update = (patch: Partial<BackupSchedule>) =>
    setList(list.map((s) => (s.id === current ? { ...s, ...patch } : s)));
  const add = () => {
    const s = newSchedule(firstConnection);
    setList([...list, s]);
    setCurrent(s.id);
  };
  const save = async () => {
    try {
      await store.get().saveSettings({ backupSchedules: list });
      onClose();
    } catch (e) {
      showError(e);
    }
  };

  const footer = (
    <>
      <button className="btn sm" onClick={add} disabled={!firstConnection}>
        <Plus size={13} /> New schedule
      </button>
      <span className="spacer" />
      <button className="btn" onClick={onClose}>
        Cancel
      </button>
      <button className="btn primary" onClick={() => void save()}>
        Save
      </button>
    </>
  );

  return (
    <Modal
      title="Backup schedules"
      icon={<CalendarClock size={18} />}
      size="lg"
      onClose={onClose}
      footer={footer}
    >
      <div className="faint small">
        Scheduled backups run while LibraTable is open (PostgreSQL, MySQL and SQLite), each database
        to a dated file in the folder.
      </div>
      <div className="schedule-list" role="list" aria-label="Schedules">
        {list.length === 0 && <div className="faint small pad">No backup schedules yet.</div>}
        {list.map((s) => (
          <div
            key={s.id}
            role="listitem"
            className={`schedule-row${s.id === current ? " selected" : ""}`}
            onClick={() => setCurrent(s.id)}
          >
            <div className="grow">
              <div>{s.name || store.connection(s.connectionId)?.name || "Backup"}</div>
              <div className="faint small">{status(s)}</div>
            </div>
            <button
              className="btn sm icon ghost"
              aria-label="Run now"
              title="Run now"
              disabled={!s.folder}
              onClick={(e) => {
                e.stopPropagation();
                void runAndRecord(s);
              }}
            >
              <Play size={13} />
            </button>
            <button
              className="btn sm icon ghost"
              aria-label="Delete schedule"
              onClick={(e) => {
                e.stopPropagation();
                setList(list.filter((x) => x.id !== s.id));
              }}
            >
              <Trash2 size={13} />
            </button>
          </div>
        ))}
      </div>
      {selected && <Editor s={selected} onChange={update} />}
    </Modal>
  );
}
