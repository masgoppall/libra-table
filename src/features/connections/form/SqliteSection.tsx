import { FilePlus2 } from "lucide-react";
import { PathField } from "./fields";
import { type ConnectionFormState } from "./useConnectionForm";

export function SqliteSection({ form }: { form: ConnectionFormState }) {
  return (
    <div className="form-grid form-section">
      <label>Database file</label>
      <div className="row">
        <div className="grow">
          <PathField
            value={form.c.db.filePath}
            onChange={(filePath) => form.setDb({ filePath })}
            placeholder="/path/to/database.sqlite"
            title="Choose SQLite file"
          />
        </div>
        <button
          className="btn"
          onClick={form.createSqliteFile}
          title="Create an empty database file"
        >
          <FilePlus2 size={14} /> New file…
        </button>
      </div>
    </div>
  );
}
