// Step 1: the file, its format and CSV options, and a preview of the first rows.

import { FolderOpen } from "lucide-react";
import { Check } from "@/components/Check";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Spinner } from "@/components/Spinner";
import { formatBytes, plural } from "@/lib/format";
import { type CsvOptions } from "@/types";
import { PreviewTable } from "./PreviewTable";
import { type ImportWizardState } from "./useImportWizard";

const DELIMITERS: [string, string][] = [
  [",", "Comma ,"],
  [";", "Semicolon ;"],
  ["\t", "Tab"],
  ["|", "Pipe |"],
];

const QUOTES: [string, string][] = [
  ['"', 'Double "'],
  ["'", "Single '"],
  ["", "None"],
];

function Choice(props: {
  value: string;
  options: [string, string][];
  onChange: (v: string) => void;
  label: string;
}) {
  return (
    <select
      className="select"
      aria-label={props.label}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
    >
      {props.options.map(([v, label]) => (
        <option key={label} value={v}>
          {label}
        </option>
      ))}
    </select>
  );
}

function CsvFields({ csv, set }: { csv: CsvOptions; set: (p: Partial<CsvOptions>) => void }) {
  return (
    <>
      <label>Delimiter</label>
      <div className="row">
        <Choice
          label="Delimiter"
          value={csv.delimiter}
          options={DELIMITERS}
          onChange={(delimiter) => set({ delimiter })}
        />
        <span className="muted">Quote</span>
        <Choice
          label="Quote"
          value={csv.quote}
          options={QUOTES}
          onChange={(q) => set({ quote: q })}
        />
        <span className="muted">Encoding</span>
        <Choice
          label="Encoding"
          value={csv.encoding}
          options={[
            ["utf8", "UTF-8"],
            ["latin1", "Latin-1"],
          ]}
          onChange={(v) => set({ encoding: v as CsvOptions["encoding"] })}
        />
      </div>
      <label>Rows</label>
      <div className="row">
        <Check checked={csv.header} onChange={(header) => set({ header })}>
          Column names in first row
        </Check>
        <span className="muted">Skip first</span>
        <input
          className="input num-input"
          type="number"
          min={0}
          aria-label="Rows to skip"
          value={csv.skipRows}
          onChange={(e) => set({ skipRows: Math.max(0, Number(e.target.value) || 0) })}
        />
        <span className="muted">rows</span>
      </div>
      <label>NULL</label>
      <div className="row">
        <input
          className="input mono import-null"
          aria-label="NULL marker"
          placeholder="none"
          value={csv.nullMarker}
          onChange={(e) => set({ nullMarker: e.target.value })}
        />
        <Check checked={csv.emptyAsNull} onChange={(emptyAsNull) => set({ emptyAsNull })}>
          Empty fields are NULL
        </Check>
      </div>
    </>
  );
}

export function SourceStep({ w }: { w: ImportWizardState }) {
  const { source, preview } = w;
  return (
    <>
      <div className="form-grid">
        <label>File</label>
        <div className="row">
          <input
            className="input mono grow"
            readOnly
            placeholder="Choose a CSV or JSON file"
            value={source.path}
          />
          <button className="btn" onClick={() => void w.chooseFile()}>
            <FolderOpen size={14} /> Choose file…
          </button>
        </div>
        <label>Format</label>
        <div className="row">
          {(["csv", "json"] as const).map((f) => (
            <label key={f} className="check">
              <input
                type="radio"
                name="import-format"
                checked={source.format === f}
                onChange={() => w.setFormat(f)}
              />
              {f.toUpperCase()}
            </label>
          ))}
          {source.format === "json" && (
            <span className="faint small">An array of objects, or one object per line</span>
          )}
        </div>
        {source.format === "csv" && <CsvFields csv={source.csv} set={w.setCsv} />}
      </div>
      <ErrorCallout error={w.previewError} />
      {w.loading && !preview && (
        <div className="row muted">
          <Spinner /> Reading the file…
        </div>
      )}
      {preview && (
        <>
          <div className="faint small">
            {formatBytes(preview.totalBytes)} · {plural(preview.columns.length, "column")} · types
            inferred from the first {plural(preview.sampled, "row")}
          </div>
          <PreviewTable preview={preview} />
          {preview.errors.length > 0 && (
            <div className="callout warn">
              {plural(preview.errors.length, "row")} of the sample can't be read, e.g. row{" "}
              {preview.errors[0]?.row}: {preview.errors[0]?.message}
            </div>
          )}
        </>
      )}
    </>
  );
}
