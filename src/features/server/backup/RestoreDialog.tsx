// Restore a .sql script (psql / mysql) or a pg_dump archive (pg_restore) into a database.
// The command is confirmed first (in red on production), then progress streams in.

import { open } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { isProduction } from "@/domain/connection";
import { toastInfo, toastSuccess } from "@/services/errors";
import { store, tabDb } from "@/store";
import { type RestoreRequest, type Workspace } from "@/types";
import { defaultRestore, progressText, restoreFilters } from "./backupModel";
import { CommandPreview, FileField, JobLog } from "./JobPanel";
import { ToolField } from "./ToolField";
import { useToolJob } from "./useToolJob";
import { useToolPlan } from "./useToolPlan";

async function confirmRestore(ws: Workspace, database: string, command: string) {
  const conn = store.connection(ws.connectionId);
  const prod = isProduction(conn);
  const r = await store.get().dialog({
    kind: "confirm",
    title: prod ? `Production: restore into ${database}` : `Restore into ${database}`,
    message:
      "The file's statements run in this database and can overwrite or drop what is there. " +
      "This command will run:",
    details: [command],
    okLabel: "Restore",
    danger: true,
  });
  return r.ok;
}

function RestoreOptions(props: {
  r: RestoreRequest;
  set: (p: Partial<RestoreRequest>) => void;
  tool?: string;
}) {
  const { r, set, tool } = props;
  return (
    <div className="check-list">
      <Check checked={r.stopOnError} onChange={(stopOnError) => set({ stopOnError })}>
        Stop at the first error
      </Check>
      {(tool === "psql" || tool === "pg_restore") && (
        <Check
          checked={r.singleTransaction}
          onChange={(singleTransaction) => set({ singleTransaction })}
        >
          All or nothing (one transaction)
        </Check>
      )}
      {tool === "pg_restore" && (
        <>
          <Check checked={r.clean} onChange={(clean) => set({ clean })}>
            Drop objects before recreating them (--clean)
          </Check>
          <Check checked={r.noOwner} onChange={(noOwner) => set({ noOwner })}>
            Don&apos;t set owners (--no-owner)
          </Check>
        </>
      )}
    </div>
  );
}

export function RestoreDialog(props: { ws: Workspace; database: string; onClose: () => void }) {
  const { ws, database } = props;
  const id = ws.connectionId;
  const db = tabDb(ws, { database });
  const [r, setR] = useState<RestoreRequest>(defaultRestore());
  const set = (patch: Partial<RestoreRequest>) => setR((x) => ({ ...x, ...patch }));
  const plan = useToolPlan(r, (req) => api.planRestore(id, db, req));
  const job = useToolJob({
    onDone: () => {
      toastSuccess(`Restored into ${database}.`, r.path);
      store.get().refresh(id);
      props.onClose();
    },
    onCancelled: () => toastInfo("Restore cancelled."),
  });
  const running = !!job.job;
  const tool = plan.plan?.tool;

  const choose = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: restoreFilters(ws.info.driver),
    });
    if (typeof path === "string") set({ path });
  };
  const start = () =>
    void job.start(async (jobId) => {
      const command = plan.plan?.command ?? "";
      if (!(await confirmRestore(ws, database, command))) return false;
      await api.startRestore(id, db, jobId, r, true);
      return true;
    });

  const footer = running ? (
    <>
      <span className="row muted">
        <Spinner /> {progressText(job.job?.bytes ?? 0, job.job?.total ?? null, "read")}
      </span>
      <span className="spacer" />
      <button className="btn" onClick={job.cancel}>
        Cancel restore
      </button>
    </>
  ) : (
    <>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Close
      </button>
      <button className="btn primary" onClick={start} disabled={!r.path || !tool?.path}>
        Restore…
      </button>
    </>
  );
  return (
    <Modal
      title={`Restore · ${database}`}
      size="lg"
      icon={<ArchiveRestore size={18} />}
      onClose={() => !running && props.onClose()}
      footer={footer}
    >
      <div className="form-grid">
        <label>File</label>
        <FileField path={r.path} onChoose={() => void choose()} disabled={running} />
        <label>Options</label>
        <RestoreOptions r={r} set={set} tool={tool?.tool} />
        {tool && <ToolField tool={tool} onChanged={plan.reload} disabled={running} />}
      </div>
      <CommandPreview command={r.path ? (plan.plan?.command ?? "") : ""} />
      <JobLog lines={job.lines} />
      <ErrorCallout error={job.error ?? plan.error} />
    </Modal>
  );
}
