// Format and options of an export: CSV delimiter / NULL / header / BOM, SQL table name.

import { Check } from "@/components/Check";
import { type ExportFormat, type ExportOptions } from "@/types";

export const FORMATS: ExportFormat[] = ["csv", "json", "sql"];

const DELIMITERS: [string, string][] = [
  [",", "Comma ,"],
  [";", "Semicolon ;"],
  ["\t", "Tab"],
  ["|", "Pipe |"],
];

export function formatLabel(f: ExportFormat): string {
  return f === "sql" ? "SQL INSERT" : f.toUpperCase();
}

function CsvOptions(props: { opts: ExportOptions; set: (p: Partial<ExportOptions>) => void }) {
  const { opts, set } = props;
  return (
    <>
      <label>Delimiter</label>
      <div className="row">
        <select
          className="select"
          value={opts.delimiter}
          onChange={(e) => set({ delimiter: e.target.value })}
        >
          {DELIMITERS.map(([v, label]) => (
            <option key={label} value={v}>
              {label}
            </option>
          ))}
        </select>
        <span className="muted">NULL as</span>
        <select
          className="select"
          value={opts.nullAs}
          onChange={(e) => set({ nullAs: e.target.value as ExportOptions["nullAs"] })}
        >
          <option value="empty">empty</option>
          <option value="null">NULL</option>
        </select>
      </div>
      <label>Options</label>
      <div className="row">
        <Check checked={opts.header} onChange={(header) => set({ header })}>
          Column names in first row
        </Check>
        <Check checked={opts.bom} onChange={(bom) => set({ bom })}>
          UTF-8 BOM (Excel)
        </Check>
      </div>
    </>
  );
}

export function ExportOptionsForm(props: {
  format: ExportFormat;
  onFormat: (f: ExportFormat) => void;
  opts: ExportOptions;
  onOpts: (o: ExportOptions) => void;
  disabled: boolean;
  /** Several tables: each SQL file inserts into its own table. */
  hideTableName?: boolean;
}) {
  const set = (patch: Partial<ExportOptions>) => props.onOpts({ ...props.opts, ...patch });
  return (
    <>
      <label>Format</label>
      <div className="row">
        {FORMATS.map((f) => (
          <label key={f} className="check">
            <input
              type="radio"
              name="fmt"
              checked={props.format === f}
              onChange={() => props.onFormat(f)}
              disabled={props.disabled}
            />
            {formatLabel(f)}
          </label>
        ))}
      </div>
      {props.format === "csv" && <CsvOptions opts={props.opts} set={set} />}
      {props.format === "sql" && !props.hideTableName && (
        <>
          <label>Table name</label>
          <input
            className="input mono"
            value={props.opts.tableName}
            onChange={(e) => set({ tableName: e.target.value })}
          />
        </>
      )}
    </>
  );
}
