// Run a SQL file (ImportSQLWizard): Rust streams the file statement by statement.

import { open } from "@tauri-apps/plugin-dialog";
import { FileCode2, FolderOpen } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { formatBytes, plural } from "@/lib/format";
import { withSafeMode } from "@/services/safeMode";
import { store, type ToolDialog } from "@/store";
import { type SqlFileProgress, type SqlFileSummary } from "@/types";
import { progressPercent } from "./importModel";
import { ProgressBar } from "./RunStep";
import { useBackgroundJob } from "./useBackgroundJob";

type SqlDialog = Extract<ToolDialog, { kind: "sql-file" }>;

function summaryText(s: SqlFileSummary): { tone: string; text: string } {
  const ran = `${plural(s.statements, "statement")} run, ${s.failed} failed`;
  const skipped = s.skippedCommands ? `; ${plural(s.skippedCommands, "psql command")} skipped` : "";
  if (s.fatal)
    return { tone: "error", text: `${ran}. The file could not be read: ${s.fatal.message}` };
  if (s.cancelled) return { tone: "warn", text: `Cancelled after ${ran}.` };
  if (s.stopped) return { tone: "error", text: `Stopped at the first error: ${ran}.` };
  return { tone: s.failed ? "warn" : "ok", text: `${ran}${skipped}.` };
}

function Summary({ s }: { s: SqlFileSummary }) {
  const { tone, text } = summaryText(s);
  return (
    <>
      <div className={`callout ${tone}`} role="status">
        {text}
      </div>
      {s.errors.length > 0 && (
        <div className="import-errors statement-list">
          {s.errors.map((e) => (
            <div key={e.index} className="selectable">
              <div className="small">
                Statement {e.index} (line {e.line}): <span className="warn-text">{e.message}</span>
              </div>
              <pre className="mono small">{e.sql}</pre>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function Progress({ p }: { p: SqlFileProgress | null }) {
  return (
    <>
      <div className="row muted">
        <Spinner />
        {p
          ? `${plural(p.statements, "statement")} run · ${p.failed} failed · ` +
            `${formatBytes(p.bytesRead)} of ${formatBytes(p.totalBytes)}`
          : "Starting…"}
      </div>
      <ProgressBar percent={p ? progressPercent(p) : 0} />
    </>
  );
}

export function SqlFileDialog({ dialog, onClose }: { dialog: SqlDialog; onClose: () => void }) {
  const [path, setPath] = useState("");
  const [stopOnError, setStopOnError] = useState(true);
  const job = useBackgroundJob<SqlFileProgress, SqlFileSummary>("sql-file-progress", () =>
    store.get().refresh(dialog.connectionId),
  );
  const choose = async () => {
    const p = await open({ multiple: false, filters: [{ name: "SQL", extensions: ["sql"] }] });
    if (typeof p === "string") setPath(p);
  };
  const run = () =>
    job.start((jobId) => {
      const name = store.connection(dialog.connectionId)?.name ?? "";
      const req = { path, stopOnError };
      return withSafeMode(name, (confirmed) =>
        api.startSqlFile(dialog.connectionId, dialog.db, jobId, req, confirmed).then(() => true),
      ).then((ok) => ok === true);
    });
  const footer = job.running ? (
    <>
      <span className="spacer" />
      <button className="btn" onClick={job.cancel}>
        Cancel
      </button>
    </>
  ) : (
    <>
      <span className="spacer" />
      <button className="btn" onClick={onClose}>
        Close
      </button>
      <button className="btn primary" disabled={!path} onClick={() => void run()}>
        Run file
      </button>
    </>
  );
  return (
    <Modal
      title="Run SQL file"
      subtitle={dialog.db ?? undefined}
      icon={<FileCode2 size={16} />}
      onClose={() => !job.running && onClose()}
      footer={footer}
    >
      <div className="form-grid">
        <label>File</label>
        <div className="row">
          <input
            className="input mono grow"
            readOnly
            placeholder="Choose a .sql file"
            value={path}
          />
          <button className="btn" onClick={() => void choose()} disabled={job.running}>
            <FolderOpen size={14} /> Choose file…
          </button>
        </div>
        <label>Errors</label>
        <Check checked={stopOnError} onChange={setStopOnError}>
          Stop at the first error (otherwise run the rest and report)
        </Check>
      </div>
      <div className="faint small">
        Statements run in order on one connection. MySQL DELIMITER lines and Postgres COPY blocks
        from dumps are understood.
      </div>
      {job.running && <Progress p={job.progress} />}
      {job.summary && <Summary s={job.summary} />}
      <ErrorCallout error={job.error} />
    </Modal>
  );
}
