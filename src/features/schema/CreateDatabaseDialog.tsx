// "New database": name and server options, with the SQL previewed as you type.

import { Database } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { isProduction } from "@/domain/connection";
import { useDebouncedEffect } from "@/hooks/useDebouncedEffect";
import { toastSuccess } from "@/services/errors";
import { writeDialogOptions } from "@/services/safeMode";
import { store, useConnection } from "@/store";
import { type AppError, type DatabaseOptions, type SchemaChange, type Workspace } from "@/types";
import { DatabaseOptionsFields, type OptionsDraft } from "./DatabaseOptionsFields";

const PREVIEW_DELAY_MS = 150;

function toOptions(pg: boolean, o: OptionsDraft): DatabaseOptions {
  if (pg) return { encoding: o.encoding, template: o.template, owner: o.owner };
  return { charset: o.charset, collation: o.collation };
}

function SqlPreview({ sql }: { sql: string[] }) {
  if (!sql.length) return null;
  return (
    <div className="sql-box">
      <div className="sql-box-head">SQL</div>
      <pre className="mono selectable">{sql.join(";\n")};</pre>
    </div>
  );
}

export function CreateDatabaseDialog({ ws, onClose }: { ws: Workspace; onClose: () => void }) {
  const id = ws.connectionId;
  const conn = useConnection(id);
  const [name, setName] = useState("");
  const [opts, setOpts] = useState<OptionsDraft>({
    charset: "utf8mb4",
    collation: "",
    encoding: "",
    template: "",
    owner: "",
  });
  const [sql, setSql] = useState<string[]>([]);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const pg = ws.info.driver === "postgres";
  const trimmed = name.trim();
  const change: SchemaChange = {
    kind: "create_database",
    name: trimmed,
    options: toOptions(pg, opts),
  };

  useDebouncedEffect(
    () => {
      if (!trimmed) return setSql([]);
      api
        .previewSchemaChange(id, null, change)
        .then((s) => {
          setSql(s);
          setError(null);
        })
        .catch((e) => setError(e as AppError));
    },
    [name, opts],
    PREVIEW_DELAY_MS,
  );

  // Safe mode: production (or a stricter level) confirms the SQL once more.
  const confirmWrite = async () => {
    const guard = writeDialogOptions(id);
    if (!guard.danger) return true;
    const prod = isProduction(conn);
    const r = await store.get().dialog({
      kind: "confirm",
      title: prod ? `Production: ${conn?.name}` : `Confirm on ${conn?.name}`,
      message: "Create this database on the server?",
      details: sql,
      okLabel: prod ? "Create on production" : "Create database",
      ...guard,
    });
    return r.ok;
  };

  const submit = async () => {
    if (!trimmed || busy || !(await confirmWrite())) return;
    setBusy(true);
    try {
      await api.applySchemaChange(id, null, change, true);
      toastSuccess(`Created database "${trimmed}".`);
      onClose();
      store.get().switchDatabase(id, trimmed);
      store.get().refresh(id);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="New database"
      subtitle={conn ? `on ${conn.name}` : undefined}
      icon={<Database size={18} />}
      onClose={onClose}
      footer={
        <ModalActions
          onCancel={onClose}
          onOk={submit}
          okLabel="Create database"
          busy={busy}
          disabled={!trimmed || !!error}
        />
      }
    >
      <form
        className="form-grid"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <label>Name</label>
        <input
          className="input"
          autoFocus
          value={name}
          placeholder="e.g. shop_dev"
          onChange={(e) => setName(e.target.value)}
        />
        <DatabaseOptionsFields postgres={pg} value={opts} onChange={setOpts} />
        <button type="submit" hidden />
      </form>
      <SqlPreview sql={sql} />
      <ErrorCallout error={error} />
    </Modal>
  );
}
