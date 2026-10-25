// Backup with pg_dump / mysqldump (SQLite: VACUUM INTO). The command is shown before it
// runs; progress streams from Rust as "tool-progress" events.

import { save } from "@tauri-apps/plugin-dialog";
import { DatabaseBackup } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { formatBytes } from "@/lib/format";
import { toastInfo, toastSuccess } from "@/services/errors";
import { tabDb } from "@/store";
import { type BackupRequest, type Workspace } from "@/types";
import { backupExtension, backupFileName, defaultBackup, progressText } from "./backupModel";
import { BackupOptions } from "./BackupOptions";
import { CommandPreview, FileField, JobLog } from "./JobPanel";
import { ToolField } from "./ToolField";
import { useToolJob } from "./useToolJob";
import { useToolPlan } from "./useToolPlan";

export function BackupDialog(props: { ws: Workspace; database: string; onClose: () => void }) {
  const { ws, database } = props;
  const id = ws.connectionId;
  const db = tabDb(ws, { database });
  const driver = ws.info.driver;
  const [r, setR] = useState<BackupRequest>(defaultBackup());
  const set = (patch: Partial<BackupRequest>) => setR((x) => ({ ...x, ...patch }));
  const plan = useToolPlan(r, (req) => api.planBackup(id, db, req));
  const job = useToolJob({
    onDone: (p) => {
      toastSuccess(`Backup of ${database} written (${formatBytes(p.bytes)}).`, p.path);
      props.onClose();
    },
    onCancelled: () => toastInfo("Backup cancelled."),
  });
  const running = !!job.job;

  const choose = async () => {
    const ext = backupExtension(driver, r.format);
    const path = await save({
      defaultPath: backupFileName(driver === "sqlite" ? ws.info.database : database, ext),
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (path) set({ path });
  };
  const start = () => void job.start((jobId) => api.startBackup(id, db, jobId, r).then(() => true));

  const footer = running ? (
    <>
      <span className="row muted">
        <Spinner /> {progressText(job.job?.bytes ?? 0, null, "written")}…
      </span>
      <span className="spacer" />
      <button className="btn" onClick={job.cancel}>
        Cancel backup
      </button>
    </>
  ) : (
    <>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Close
      </button>
      <button
        className="btn primary"
        onClick={start}
        disabled={!r.path || (driver !== "sqlite" && !plan.plan?.tool?.path)}
      >
        Back up
      </button>
    </>
  );
  const tool = plan.plan?.tool;
  return (
    <Modal
      title={`Backup · ${database}`}
      size="lg"
      icon={<DatabaseBackup size={18} />}
      onClose={() => !running && props.onClose()}
      footer={footer}
    >
      <div className="form-grid">
        <label>Save to</label>
        <FileField path={r.path} onChoose={() => void choose()} disabled={running} />
        {driver !== "sqlite" && (
          <BackupOptions ws={ws} db={db} database={database} r={r} set={set} />
        )}
        {tool && <ToolField tool={tool} onChanged={plan.reload} disabled={running} />}
      </div>
      {driver === "sqlite" && (
        <div className="faint small">
          SQLite writes a compact copy of the open database with VACUUM INTO. To restore it, open
          the copy as a connection.
        </div>
      )}
      <CommandPreview command={r.path ? (plan.plan?.command ?? "") : ""} />
      <JobLog lines={job.lines} />
      <ErrorCallout error={job.error ?? plan.error} />
    </Modal>
  );
}
