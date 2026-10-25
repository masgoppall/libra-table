// Export (EXP-1..EXP-4): Rust streams rows straight to the file and reports progress.

import { useState } from "react";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { formatCount } from "@/lib/format";
import {
  type Db,
  type ExportFormat,
  type ExportOptions,
  type ExportSource,
  type PageRequest,
  type Workspace,
} from "@/types";
import { ExportOptionsForm } from "./ExportOptionsForm";
import { useExportJob } from "./useExportJob";

type Scope = "all" | "page" | "query";

export interface ExportDialogProps {
  ws: Workspace;
  /** Database the table or query is in. */
  db: Db;
  defaultName: string;
  tableSource?: { request: PageRequest; hasFilters: boolean };
  querySql?: string;
  onClose: () => void;
}

function exportSource(p: ExportDialogProps, scope: Scope): ExportSource {
  if (scope === "query" || !p.tableSource) return { kind: "query", sql: p.querySql ?? "" };
  return { kind: "table", pageOnly: scope === "page", ...p.tableSource.request };
}

function ScopeSelect(props: {
  p: ExportDialogProps;
  scope: Scope;
  onScope: (s: Scope) => void;
  disabled: boolean;
}) {
  const { tableSource, querySql } = props.p;
  return (
    <select
      className="select"
      value={props.scope}
      onChange={(e) => props.onScope(e.target.value as Scope)}
      disabled={props.disabled}
    >
      {tableSource && (
        <>
          <option value="all">
            All rows{tableSource.hasFilters ? " matching the filters" : ""}
          </option>
          <option value="page">This page only</option>
        </>
      )}
      {querySql !== undefined && <option value="query">Query result</option>}
    </select>
  );
}

export function ExportDialog(props: ExportDialogProps) {
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [scope, setScope] = useState<Scope>(props.tableSource ? "all" : "query");
  const [opts, setOpts] = useState<ExportOptions>({
    delimiter: ",",
    quote: '"',
    header: true,
    bom: false,
    nullAs: "empty",
    tableName: props.defaultName,
  });
  const exp = useExportJob(props.ws.connectionId, props.db, props.onClose);
  const job = exp.job;
  const start = () =>
    exp.start(props.defaultName, format, {
      source: exportSource(props, scope),
      format,
      options: opts,
    });

  const footer = job ? (
    <>
      <span className="row muted">
        <Spinner /> {formatCount(job.rows)} rows written…
      </span>
      <span className="spacer" />
      <button className="btn" onClick={exp.cancel}>
        Cancel export
      </button>
    </>
  ) : (
    <>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Close
      </button>
      <button className="btn primary" onClick={() => void start()}>
        Export…
      </button>
    </>
  );

  return (
    <Modal title="Export" onClose={() => !job && props.onClose()} footer={footer}>
      <div className="form-grid">
        <label>Rows</label>
        <ScopeSelect p={props} scope={scope} onScope={setScope} disabled={!!job} />
        <ExportOptionsForm
          format={format}
          onFormat={setFormat}
          opts={opts}
          onOpts={setOpts}
          disabled={!!job}
        />
      </div>
      <div className="faint small">
        Rows stream from the database to the file, so large exports don't fill memory. Binary values
        are exported in full.
      </div>
      <ErrorCallout error={exp.error} />
    </Modal>
  );
}
