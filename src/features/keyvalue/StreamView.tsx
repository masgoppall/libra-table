// Stream entries, read-only: id and the field/value pairs.

import { valueText } from "./edits";
import { type StreamEntry } from "./types";

export function StreamView({ entries }: { entries: StreamEntry[] }) {
  if (!entries.length) return <div className="message faint">The stream is empty.</div>;
  return (
    <table className="edit-table kv-table">
      <thead>
        <tr>
          <th>ID</th>
          <th>Fields</th>
        </tr>
      </thead>
      <tbody>
        {entries.map((e) => (
          <tr key={e.id}>
            <td className="kv-cell mono">{e.id}</td>
            <td className="kv-cell">
              {e.fields.map(([f, v], i) => (
                <div key={i}>
                  <span className="muted">{valueText(f)}</span> {valueText(v)}
                </div>
              ))}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
