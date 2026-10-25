// A string value: edit as text, or view it as a JSON tree. Binary and very long strings
// are shown read-only.

import { Save } from "lucide-react";
import { useEffect, useState } from "react";
import { JsonTree, parseJson } from "@/components/JsonTree";
import { type KeyTarget, runEdit, valueText } from "./edits";
import { type KeyData, type RedisValue } from "./types";

type StringData = Extract<KeyData, { kind: "string" }>;

export function StringEditor(p: {
  target: KeyTarget;
  keyName: RedisValue;
  canEdit: boolean;
  value: StringData;
  onSaved: () => void;
}) {
  const original = p.value.text ?? "";
  const [text, setText] = useState(original);
  const [view, setView] = useState<"text" | "json">("text");
  useEffect(() => setText(original), [original]);
  const json = view === "json" ? parseJson(text) : null;
  const editable = p.canEdit && p.value.text !== null && !p.value.truncated;

  if (p.value.text === null) {
    return (
      <div className="col tight">
        <div className="muted">Binary value (shown as hex, read-only here).</div>
        <pre className="kv-pre mono">{valueText(p.value.binary)}</pre>
      </div>
    );
  }
  const save = async () => {
    const edit = { op: "set_string" as const, key: p.keyName, value: text };
    if (await runEdit(p.target, edit, "Saved.")) p.onSaved();
  };
  return (
    <div className="col tight grow">
      <div className="row tight">
        <div className="segmented">
          <button className={view === "text" ? "active" : ""} onClick={() => setView("text")}>
            Text
          </button>
          <button className={view === "json" ? "active" : ""} onClick={() => setView("json")}>
            JSON
          </button>
        </div>
        <span className="spacer" />
        {editable && (
          <button
            className="btn xs primary"
            disabled={text === original}
            onClick={() => void save()}
          >
            <Save size={12} /> Save
          </button>
        )}
      </div>
      {p.value.truncated && (
        <div className="warn-text small">Only the first 4 MB are shown; edit it in a query.</div>
      )}
      {view === "text" ? (
        <textarea
          className="input kv-text mono"
          aria-label="String value"
          value={text}
          readOnly={!editable}
          onChange={(e) => setText(e.target.value)}
        />
      ) : json !== null ? (
        <div className="kv-json">
          <JsonTree value={json} />
        </div>
      ) : (
        <div className="message faint">This value is not valid JSON.</div>
      )}
    </div>
  );
}
