// Export several tables at once (ExportTablesWindow): one file per table in a folder.

import { open } from "@tauri-apps/plugin-dialog";
import { Download, FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type ToolDialog } from "@/store";
import {
  type ExportFormat,
  type ExportOptions,
  type ExportTablesEvent,
  type ExportTablesProgress,
  type AppError,
} from "@/types";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { uid } from "@/lib/uid";
import { ExportOptionsForm } from "./ExportOptionsForm";

type TablesDialog = Extract<ToolDialog, { kind: "export-tables" }>;

const OPTIONS: ExportOptions = {
  delimiter: ",",
  quote: '"',
  header: true,
  bom: false,
  nullAs: "empty",
  tableName: "",
};

function useTables(d: TablesDialog) {
  const [tables, setTables] = useState<string[] | null>(null);
  useEffect(() => {
    api
      .listObjects(d.connectionId, d.db, d.schema)
      .then((os) => setTables(os.filter((o) => o.kind === "table").map((o) => o.name)))
      .catch(() => setTables([]));
  }, [d.connectionId, d.schema]);
  return tables;
}

/** The job: start it, follow `export-tables-progress`, cancel it. */
function useTablesJob(onDone: (p: ExportTablesProgress) => void) {
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ExportTablesProgress | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  useTauriEvent<ExportTablesEvent>("export-tables-progress", (e) => {
    if (e.jobId !== jobId) return;
    if (!e.done) return setProgress(e.progress);
    setJobId(null);
    if (e.error) setError(e.error);
    else onDone(e.progress);
  });
  const start = (run: (id: string) => Promise<void>) => {
    const id = uid();
    setJobId(id);
    setError(null);
    setProgress(null);
    run(id).catch((e) => (setJobId(null), setError(e as AppError)));
  };
  const cancel = () => jobId && void api.cancelExport(jobId);
  return { running: jobId !== null, progress, error, start, cancel };
}

function TableList(props: {
  tables: string[];
  selected: string[];
  onChange: (s: string[]) => void;
}) {
  const { tables, selected } = props;
  const all = tables.length > 0 && selected.length === tables.length;
  const toggle = (t: string, on: boolean) =>
    props.onChange(
      on ? tables.filter((x) => x === t || selected.includes(x)) : selected.filter((x) => x !== t),
    );
  return (
    <div className="export-tables">
      <Check checked={all} onChange={(on) => props.onChange(on ? tables : [])}>
        <strong>All tables</strong> <span className="faint">({tables.length})</span>
      </Check>
      <div className="export-tables-list">
        {tables.map((t) => (
          <Check key={t} checked={selected.includes(t)} onChange={(on) => toggle(t, on)}>
            <span className="mono">{t}</span>
          </Check>
        ))}
      </div>
    </div>
  );
}

export function ExportTablesDialog(props: { dialog: TablesDialog; onClose: () => void }) {
  const d = props.dialog;
  const tables = useTables(d);
  const [selected, setSelected] = useState<string[]>(d.selected);
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [opts, setOpts] = useState(OPTIONS);
  const [folder, setFolder] = useState("");
  // Opened without a selection: start with every table.
  useEffect(() => {
    if (tables && d.selected.length === 0) setSelected(tables);
  }, [tables]);
  const job = useTablesJob((p) => {
    toastSuccess(
      `Exported ${plural(p.files.length, "table")} (${plural(p.totalRows, "row")}).`,
      folder,
    );
    props.onClose();
  });
  const choose = async () => {
    const f = await open({ directory: true, multiple: false });
    if (typeof f === "string") setFolder(f);
  };
  const run = () =>
    job.start((id) =>
      api.startExportTables(d.connectionId, d.db, id, {
        schema: d.schema,
        tables: (tables ?? []).filter((t) => selected.includes(t)),
        format,
        folder,
        options: opts,
      }),
    );
  const p = job.progress;
  const footer = job.running ? (
    <>
      <span className="row muted">
        <Spinner />
        {p
          ? `Table ${p.index + 1} of ${p.count}: ${p.table} · ${plural(p.rows, "row")}`
          : "Starting…"}
      </span>
      <span className="spacer" />
      <button className="btn" onClick={job.cancel}>
        Cancel export
      </button>
    </>
  ) : (
    <>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Close
      </button>
      <button className="btn primary" disabled={!folder || selected.length === 0} onClick={run}>
        Export {plural(selected.length, "table")}
      </button>
    </>
  );
  return (
    <Modal
      title="Export tables"
      subtitle={d.schema}
      icon={<Download size={16} />}
      size="lg"
      onClose={() => !job.running && props.onClose()}
      footer={footer}
    >
      {tables === null ? (
        <div className="row muted">
          <Spinner /> Loading tables…
        </div>
      ) : (
        <TableList tables={tables} selected={selected} onChange={setSelected} />
      )}
      <div className="form-grid">
        <ExportOptionsForm
          format={format}
          onFormat={setFormat}
          opts={opts}
          onOpts={setOpts}
          disabled={job.running}
          hideTableName
        />
        <label>Folder</label>
        <div className="row">
          <input
            className="input mono grow"
            readOnly
            placeholder="Choose a folder"
            value={folder}
          />
          <button className="btn" onClick={() => void choose()}>
            <FolderOpen size={14} /> Choose folder…
          </button>
        </div>
      </div>
      <div className="faint small">
        One file per table, named after it; existing files with the same name are replaced.
      </div>
      <ErrorCallout error={job.error} />
    </Modal>
  );
}
