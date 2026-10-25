// Step 3: progress while the import runs, then what happened.

import { ErrorCallout } from "@/components/ErrorCallout";
import { Spinner } from "@/components/Spinner";
import { plural } from "@/lib/format";
import { type ImportProgress, type ImportSummary, type RowError } from "@/types";
import { progressLine, progressPercent, summaryOf } from "./importModel";
import { type BackgroundJob } from "./useBackgroundJob";

export function ProgressBar({ percent }: { percent: number }) {
  return (
    <div className="progress" role="progressbar" aria-valuenow={percent}>
      <div className="progress-fill" style={{ width: `${percent}%` }} />
    </div>
  );
}

export function RowErrors({ errors, total }: { errors: RowError[]; total: number }) {
  if (errors.length === 0) return null;
  return (
    <div className="import-errors">
      <table className="edit-table">
        <thead>
          <tr>
            <th className="center">Row</th>
            <th className="center">Line</th>
            <th>Error</th>
          </tr>
        </thead>
        <tbody>
          {errors.map((e, i) => (
            <tr key={i}>
              <td className="num">{e.row}</td>
              <td className="num">{e.line ?? ""}</td>
              <td className="selectable">{e.message}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {total > errors.length && (
        <div className="faint small">and {plural(total - errors.length, "more error")}</div>
      )}
    </div>
  );
}

function Summary({ s }: { s: ImportSummary }) {
  const { tone, text } = summaryOf(s);
  return (
    <>
      <div className={`callout ${tone}`} role="status">
        {text}
      </div>
      <RowErrors errors={s.errors} total={s.errorCount} />
    </>
  );
}

export function RunStep({ job }: { job: BackgroundJob<ImportProgress, ImportSummary> }) {
  const p = job.progress;
  return (
    <>
      {job.running && (
        <>
          <div className="row muted">
            <Spinner /> {p ? progressLine(p) : "Starting…"}
          </div>
          <ProgressBar percent={p ? progressPercent(p) : 0} />
        </>
      )}
      {job.summary && <Summary s={job.summary} />}
      <ErrorCallout error={job.error} />
    </>
  );
}
