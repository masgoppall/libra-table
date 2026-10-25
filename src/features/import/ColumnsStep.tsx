// Step 2: which source column goes where (or the new table's columns and types), and
// how the rows are written.

import { ArrowRight } from "lucide-react";
import { Check } from "@/components/Check";
import { type Driver } from "@/types";
import { TypeSuggestions } from "../structure/TypeSuggestions";
import { type ImportOptions, type NewTableColumn } from "./importModel";
import { type ImportWizardState } from "./useImportWizard";

function sample(w: ImportWizardState, i: number): string {
  const v = w.preview?.rows[0]?.[i];
  return v === undefined ? "" : (v ?? "NULL");
}

function Mapping({ w }: { w: ImportWizardState }) {
  const set = (i: number, target: string | null) =>
    w.setTargets(w.targets.map((t, j) => (j === i ? target : t)));
  return (
    <table className="edit-table import-map">
      <thead>
        <tr>
          <th>File column</th>
          <th />
          <th>Table column</th>
          <th>First value</th>
        </tr>
      </thead>
      <tbody>
        {(w.preview?.columns ?? []).map((c, i) => (
          <tr key={c.name}>
            <td className="mono">{c.name}</td>
            <td className="center faint">
              <ArrowRight size={13} />
            </td>
            <td>
              <select
                className="select sm"
                aria-label={`Target of ${c.name}`}
                value={w.targets[i] ?? ""}
                onChange={(e) => set(i, e.target.value || null)}
              >
                <option value="">— skip —</option>
                {w.targetColumns.map((t) => (
                  <option key={t.name} value={t.name}>
                    {t.name} ({t.dataType})
                  </option>
                ))}
              </select>
            </td>
            <td className="mono faint ellipsis">{sample(w, i)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function NewTable({ w, driver }: { w: ImportWizardState; driver: Driver }) {
  const set = (i: number, patch: Partial<NewTableColumn>) =>
    w.setNewColumns(w.newColumns.map((c, j) => (j === i ? { ...c, ...patch } : c)));
  return (
    <>
      <div className="form-grid">
        <label>Table name</label>
        <input
          className="input mono"
          aria-label="New table name"
          value={w.tableName}
          onChange={(e) => w.setTableName(e.target.value)}
        />
      </div>
      <TypeSuggestions driver={driver} />
      <table className="edit-table import-map">
        <thead>
          <tr>
            <th className="center">Import</th>
            <th>Column</th>
            <th>Type</th>
            <th>First value</th>
          </tr>
        </thead>
        <tbody>
          {w.newColumns.map((c, i) => (
            <tr key={i}>
              <td className="center">
                <input
                  type="checkbox"
                  aria-label={`Import column ${i + 1}`}
                  checked={c.include}
                  onChange={(e) => set(i, { include: e.target.checked })}
                />
              </td>
              <td>
                <input
                  className="cell-input mono"
                  aria-label={`Column ${i + 1} name`}
                  value={c.name}
                  onChange={(e) => set(i, { name: e.target.value })}
                />
              </td>
              <td>
                <input
                  className="cell-input mono"
                  aria-label={`Column ${i + 1} type`}
                  list={`lt-types-${driver}`}
                  value={c.dataType}
                  onChange={(e) => set(i, { dataType: e.target.value })}
                />
              </td>
              <td className="mono faint ellipsis">{sample(w, i)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function RunOptions(props: { o: ImportOptions; set: (o: ImportOptions) => void }) {
  const { o, set } = props;
  return (
    <div className="form-grid">
      <label>Batch size</label>
      <div className="row">
        <input
          className="input num-input"
          type="number"
          min={1}
          aria-label="Batch size"
          value={o.batchSize}
          onChange={(e) => set({ ...o, batchSize: Number(e.target.value) || 1 })}
        />
        <span className="faint small">rows per INSERT</span>
      </div>
      <label>On error</label>
      <select
        className="select"
        aria-label="On error"
        value={o.onError}
        onChange={(e) => set({ ...o, onError: e.target.value as ImportOptions["onError"] })}
      >
        <option value="stop">Stop and roll back</option>
        <option value="skip">Skip the row and report it</option>
      </select>
      <label>Transaction</label>
      <Check
        checked={o.transaction === "batch"}
        onChange={(b) => set({ ...o, transaction: b ? "batch" : "run" })}
      >
        Commit after every batch (otherwise one transaction for the whole file)
      </Check>
    </div>
  );
}

export function ColumnsStep({ w, driver }: { w: ImportWizardState; driver: Driver }) {
  return (
    <>
      {w.isNew ? <NewTable w={w} driver={driver} /> : <Mapping w={w} />}
      <RunOptions o={w.options} set={w.setOptions} />
      {w.problem && <div className="faint small">{w.problem}</div>}
    </>
  );
}
