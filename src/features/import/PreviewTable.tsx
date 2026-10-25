// The first rows of the file with each column's inferred type.

import { type FilePreview } from "@/types";

const SHOWN_ROWS = 8;

export function PreviewTable({ preview }: { preview: FilePreview }) {
  return (
    <div className="import-preview">
      <table className="edit-table">
        <thead>
          <tr>
            {preview.columns.map((c) => (
              <th key={c.name}>
                {c.name} <span className="import-kind">{c.kind.replace("_", " ")}</span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {preview.rows.slice(0, SHOWN_ROWS).map((r, i) => (
            <tr key={i}>
              {preview.columns.map((c, j) => {
                const v = r[j];
                return (
                  <td key={c.name} className={v === null || v === undefined ? "null" : undefined}>
                    {v ?? "NULL"}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
