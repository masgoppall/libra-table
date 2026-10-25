// Safe mode, read-only, startup query and keep-alive (TablePlus connection "advanced").

import { Check } from "@/components/Check";
import { KEEP_ALIVE_CHOICES, SAFE_MODES } from "@/domain/safeMode";
import { type SafeMode } from "@/types";
import { FieldNote } from "./fields";
import { type ConnectionFormState } from "./useConnectionForm";

export function SafetySection({ form }: { form: ConnectionFormState }) {
  const { c, setOptions } = form;
  const o = c.options;
  const mode = SAFE_MODES.find((m) => m.id === o.safeMode) ?? SAFE_MODES[0];
  return (
    <div className="form-grid form-section">
      <label>Safe mode</label>
      <div className="row">
        <select
          className="select"
          value={o.safeMode}
          onChange={(e) => setOptions({ safeMode: e.target.value as SafeMode })}
        >
          {SAFE_MODES.map((m) => (
            <option key={m.id} value={m.id}>
              {m.label}
            </option>
          ))}
        </select>
        <Check checked={o.readOnly} onChange={(readOnly) => setOptions({ readOnly })}>
          Read-only
        </Check>
      </div>
      <FieldNote>
        {mode?.description}
        {o.readOnly && " Read-only refuses every change, also on the server session."}
      </FieldNote>
      <label>Startup query</label>
      <textarea
        className="input mono startup-query"
        rows={2}
        value={o.startupQuery}
        placeholder={c.driver === "mysql" ? "SET time_zone = '+00:00';" : "SET search_path TO app;"}
        onChange={(e) => setOptions({ startupQuery: e.target.value })}
      />
      {c.driver !== "sqlite" && (
        <>
          <label>Keep alive</label>
          <select
            className="select"
            value={o.keepAliveSecs}
            onChange={(e) => setOptions({ keepAliveSecs: Number(e.target.value) })}
          >
            {KEEP_ALIVE_CHOICES.map(([secs, label]) => (
              <option key={secs} value={secs}>
                {label}
              </option>
            ))}
          </select>
        </>
      )}
    </div>
  );
}
