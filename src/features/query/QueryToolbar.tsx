import { CircleStop, Database, Play, PlayCircle, Save } from "lucide-react";
import { withShortcut } from "@/lib/actions";
import { QueryTools, type QueryToolsProps } from "./QueryTools";

export interface QueryToolbarProps {
  running: boolean;
  production: boolean;
  saved: boolean;
  /** Databases the tab can run in; empty hides the picker. */
  databases: string[];
  database: string;
  onDatabase: (d: string) => void;
  onRunCurrent: () => void;
  onRunAll: () => void;
  onCancel: () => void;
  onSave: () => void;
  tools: QueryToolsProps;
}

function DatabasePicker(p: QueryToolbarProps) {
  return (
    <>
      <div className="tool-sep" />
      <label className="db-select" title="Database this tab runs in">
        <Database size={13} />
        <select value={p.database} onChange={(e) => p.onDatabase(e.target.value)}>
          {p.databases.map((d) => (
            <option key={d} value={d}>
              {d}
            </option>
          ))}
        </select>
      </label>
    </>
  );
}

export function QueryToolbar(p: QueryToolbarProps) {
  return (
    <div className="toolbar">
      <div className="tool-group">
        <button
          className="btn sm primary"
          onClick={p.onRunCurrent}
          disabled={p.running}
          title={withShortcut("Run the statement at the cursor, or the selection", "run-current")}
        >
          <Play size={13} /> Run
        </button>
        <button
          className="btn sm"
          onClick={p.onRunAll}
          disabled={p.running}
          title={withShortcut("Run all", "run-all")}
        >
          <PlayCircle size={13} /> Run all
        </button>
        {p.running && (
          <button
            className="btn sm danger-outline"
            onClick={p.onCancel}
            title={withShortcut("Stop the query on the server", "cancel-query")}
          >
            <CircleStop size={13} /> Cancel
          </button>
        )}
      </div>
      <div className="tool-sep" />
      <QueryTools {...p.tools} />
      {p.databases.length > 0 && <DatabasePicker {...p} />}
      <span className="spacer" />
      {p.production && <span className="badge production">production</span>}
      <button className="btn sm" onClick={p.onSave} title={withShortcut("Save", "save")}>
        <Save size={13} /> {p.saved ? "Save" : "Save as…"}
      </button>
    </div>
  );
}
