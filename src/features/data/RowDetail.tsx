// Row detail panel: every column of one row, editable, JSON pretty-printed.

import { useState } from "react";
import { type GridColumn } from "@/components/grid/DataGrid";
import { cellText, cellValue, isBinary, isDefaultMarker, prettyJson } from "@/lib/cells";
import { type CellInput, type GridValue } from "@/types";

export interface RowDetailProps {
  columns: GridColumn[];
  row: number | null;
  getCell: (r: number, c: number) => GridValue;
  editable: boolean;
  onEdit: (c: number, v: CellInput) => void;
  onClose: () => void;
}

const LONG_TEXT = 60;
const MAX_ROWS = 12;

function typeNote(raw: GridValue, typeName: string): string {
  if (isDefaultMarker(raw)) return "DEFAULT";
  if (raw === null) return "NULL";
  if (isBinary(raw)) return `binary · ${raw.size} bytes`;
  return typeName;
}

function DetailValue(props: {
  text: string;
  isNull: boolean;
  readOnly: boolean;
  multiline: boolean;
  onCommit: (t: string) => void;
}) {
  const [value, setValue] = useState(props.text);
  const commit = () => {
    if (value === props.text) return;
    try {
      props.onCommit(value);
    } catch {
      // Invalid JSON in a JSON field: keep the text so the user can fix it.
    }
  };
  const lines = Math.min(MAX_ROWS, Math.max(3, value.split("\n").length));
  return (
    <textarea
      className="input"
      rows={props.multiline ? lines : 1}
      value={value}
      placeholder={props.isNull ? "NULL" : ""}
      readOnly={props.readOnly}
      onChange={(e) => setValue(e.target.value)}
      onBlur={commit}
    />
  );
}

function DetailField(props: RowDetailProps & { row: number; col: number }) {
  const c = props.columns[props.col];
  if (!c) return null;
  const raw = props.getCell(props.row, props.col);
  const v = cellValue(raw);
  const text = cellText(v);
  const pretty = typeof v === "string" ? prettyJson(v) : null;
  // JSON is edited pretty and saved compact.
  const commit = (t: string) => props.onEdit(props.col, pretty ? JSON.stringify(JSON.parse(t)) : t);
  return (
    <div className="detail-field">
      <label>
        <span className="detail-name">{c.name}</span>
        <span className="faint">{typeNote(raw, c.typeName)}</span>
      </label>
      <DetailValue
        key={`${props.row}-${c.name}-${text}`}
        text={pretty ?? text}
        isNull={v === null}
        readOnly={!props.editable || isBinary(v)}
        multiline={!!pretty || text.length > LONG_TEXT || text.includes("\n")}
        onCommit={commit}
      />
    </div>
  );
}

export function RowDetail(props: RowDetailProps) {
  const row = props.row;
  return (
    <div className="detail">
      <div className="detail-head">
        <span>{row === null ? "Row detail" : `Row ${row + 1}`}</span>
        <span className="spacer" />
        <button className="btn ghost icon xs" onClick={props.onClose} aria-label="Close row detail">
          ×
        </button>
      </div>
      {row === null ? (
        <div className="detail-body faint">Select a row to see its values.</div>
      ) : (
        <div className="detail-body">
          {props.columns.map((c, i) => (
            <DetailField key={c.name} {...props} row={row} col={i} />
          ))}
        </div>
      )}
    </div>
  );
}
