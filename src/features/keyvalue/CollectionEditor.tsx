// A hash, list, set or sorted set as a table: double-click a cell to change it (Enter
// saves, Escape cancels), delete rows, and add rows at the bottom.

import { Trash2 } from "lucide-react";
import { useState } from "react";
import { showError } from "@/services/errors";
import { collectionSpec } from "./collections";
import { isBinary, type KeyTarget, runEdit, valueText } from "./edits";
import { type KeyData, type KeyEdit, type RedisValue } from "./types";

type Collection = Extract<KeyData, { kind: "hash" | "list" | "set" | "zset" }>;

interface Editing {
  row: number;
  col: number;
  text: string;
}

export function CollectionEditor(p: {
  target: KeyTarget;
  keyName: RedisValue;
  canEdit: boolean;
  value: Collection;
  onSaved: () => void;
}) {
  const spec = collectionSpec(p.keyName, p.value);
  const [editing, setEditing] = useState<Editing | null>(null);
  const addable = spec.columns.map((c) => c.label !== "#");
  const [draft, setDraft] = useState<string[]>(spec.columns.map(() => ""));

  const send = async (edit: KeyEdit | string | null) => {
    if (edit === null) return false;
    if (typeof edit === "string") {
      showError({ kind: "validation", layer: "app", message: edit });
      return false;
    }
    const ok = await runEdit(p.target, edit);
    if (ok) p.onSaved();
    return ok;
  };

  const commitCell = async () => {
    if (!editing) return;
    const row = spec.rows[editing.row];
    setEditing(null);
    if (!row || editing.text === valueText(row.cells[editing.col] ?? null)) return;
    const values = row.cells.map((c, i) => (i === editing.col ? editing.text : valueText(c)));
    await send(spec.update(editing.row, values));
  };

  const add = async (action: (typeof spec.add)[number]) => {
    if (await send(action.edit(draft))) setDraft(spec.columns.map(() => ""));
  };

  const canEditCell = (r: number, c: number) => {
    const v = spec.rows[r]?.cells[c] ?? null;
    return p.canEdit && !!spec.columns[c]?.editable && !isBinary(v);
  };

  return (
    <div className="col tight">
      <table className="edit-table kv-table">
        <thead>
          <tr>
            {spec.columns.map((c) => (
              <th key={c.label}>{c.label}</th>
            ))}
            {p.canEdit && <th className="col-action" />}
          </tr>
        </thead>
        <tbody>
          {spec.rows.map((row, r) => (
            <tr key={`${row.id}-${r}`}>
              {row.cells.map((v, c) =>
                editing?.row === r && editing.col === c ? (
                  <td key={c}>
                    <input
                      className="cell-input"
                      autoFocus
                      value={editing.text}
                      onChange={(e) => setEditing({ ...editing, text: e.target.value })}
                      onBlur={() => setEditing(null)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void commitCell();
                        if (e.key === "Escape") setEditing(null);
                      }}
                    />
                  </td>
                ) : (
                  <td
                    key={c}
                    className={`kv-cell${typeof v === "number" ? " num" : ""}`}
                    title={canEditCell(r, c) ? "Double-click to edit" : undefined}
                    onDoubleClick={() =>
                      canEditCell(r, c) && setEditing({ row: r, col: c, text: valueText(v) })
                    }
                  >
                    {valueText(v)}
                  </td>
                ),
              )}
              {p.canEdit && (
                <td className="center">
                  <button
                    className="icon-btn"
                    aria-label="Remove"
                    title="Remove"
                    onClick={() => void send(spec.remove(r))}
                  >
                    <Trash2 size={12} />
                  </button>
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
      {p.canEdit && (
        <div className="row tight kv-add">
          {spec.columns.map((c, i) =>
            addable[i] ? (
              <input
                key={c.label}
                className="input sm grow"
                placeholder={c.label}
                aria-label={`New ${c.label.toLowerCase()}`}
                value={draft[i] ?? ""}
                onChange={(e) => setDraft(draft.map((d, j) => (j === i ? e.target.value : d)))}
                onKeyDown={(e) => e.key === "Enter" && spec.add[0] && void add(spec.add[0])}
              />
            ) : null,
          )}
          {spec.add.map((a) => (
            <button key={a.label} className="btn sm" onClick={() => void add(a)}>
              {a.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
