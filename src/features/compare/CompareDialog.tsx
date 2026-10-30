// Compare schema (TablePlus' "Sync database", structure only): this schema is the source;
// pick the target among open connections of the same kind. Differences are listed per
// table; the script that makes the target match opens in a query tab of the target, to
// read before running.

import { GitCompareArrows } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { diffSchemas, type SchemaDiff, syncScript } from "@/domain/schemaCompare";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";
import { store, useStore } from "@/store";
import { type AppError, type TableStructure, type Workspace } from "@/types";
import { newQueryTab } from "../workspace/tabs";
import { DiffList } from "./DiffList";
import { loadSchema } from "./loadSchema";

interface Side {
  connectionId: string;
  database: string;
  schema: string;
}

const dbOf = (ws: Workspace, database: string) =>
  database && database !== ws.info.database ? database : null;

function useChoices(target: Side, setTarget: (s: Side) => void) {
  const ws = useStore((s) => s.workspaces[target.connectionId]);
  const [databases, setDatabases] = useState<string[]>([]);
  const [schemas, setSchemas] = useState<string[]>([]);
  useEffect(() => {
    if (!ws) return;
    if (ws.caps.switchDatabase) void api.listDatabases(ws.connectionId).then(setDatabases);
    else setDatabases([ws.database]);
  }, [target.connectionId]);
  useEffect(() => {
    if (!ws?.caps.hasSchemas) return setSchemas([]);
    void api.listSchemas(ws.connectionId, dbOf(ws, target.database)).then((list) => {
      setSchemas(list);
      if (!list.includes(target.schema) && list[0]) setTarget({ ...target, schema: list[0] });
    });
  }, [target.connectionId, target.database]);
  return { databases, schemas };
}

export function CompareDialog({ ws, onClose }: { ws: Workspace; onClose: () => void }) {
  const workspaces = useStore((s) => s.workspaces);
  const peers = Object.values(workspaces).filter((w) => w.info.dialect === ws.info.dialect);
  const source: Side = { connectionId: ws.connectionId, database: ws.database, schema: ws.schema };
  const [target, setTarget] = useState<Side>(source);
  const choices = useChoices(target, setTarget);
  const [progress, setProgress] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [result, setResult] = useState<{
    diff: SchemaDiff;
    src: Record<string, TableStructure>;
  } | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [dropExtra, setDropExtra] = useState(false);
  const targetWs = workspaces[target.connectionId];

  const compare = async () => {
    if (!targetWs) return;
    setError(null);
    setResult(null);
    try {
      setProgress("Reading the source…");
      const src = await loadSchema(
        ws.connectionId,
        dbOf(ws, source.database),
        source.schema,
        (d, t) => setProgress(`Reading the source… ${d} / ${t}`),
      );
      const tgt = await loadSchema(
        target.connectionId,
        dbOf(targetWs, target.database),
        target.schema,
        (d, t) => setProgress(`Reading the target… ${d} / ${t}`),
      );
      const diff = diffSchemas(src, tgt);
      setResult({ diff, src });
      setPicked(new Set([...diff.create, ...diff.changed.map((c) => c.table), ...diff.drop]));
    } catch (e) {
      setError(e as AppError);
    } finally {
      setProgress(null);
    }
  };

  const script = useMemo(() => {
    if (!result) return "";
    const ddl = Object.fromEntries(Object.entries(result.src).map(([n, s]) => [n, s.ddl]));
    return syncScript(result.diff, {
      driver: ws.info.dialect,
      schema: target.schema,
      sourceSchema: source.schema,
      ddl,
      dropExtra,
      only: picked,
    });
  }, [result, picked, dropExtra, target.schema]);

  const openScript = () => {
    if (!targetWs) return;
    const title = `Sync from ${ws.database || "source"}`;
    store
      .get()
      .addTab(
        target.connectionId,
        newQueryTab(`${script}\n`, { title, database: target.database }),
      );
    store.get().setView(target.connectionId);
    onClose();
  };

  const same =
    target.connectionId === source.connectionId &&
    target.database === source.database &&
    target.schema === source.schema;
  const footer = (
    <>
      {progress && (
        <span className="row muted small">
          <Spinner /> {progress}
        </span>
      )}
      <span className="spacer" />
      <button className="btn" onClick={onClose}>
        Close
      </button>
      {result && script && (
        <>
          <button
            className="btn"
            onClick={() => void copyText(script).then(() => toastSuccess("Script copied."))}
          >
            Copy script
          </button>
          <button className="btn primary" onClick={openScript}>
            Open script in target
          </button>
        </>
      )}
      {!result && (
        <button
          className="btn primary"
          disabled={!!progress || same || !targetWs}
          onClick={() => void compare()}
        >
          Compare
        </button>
      )}
    </>
  );

  return (
    <Modal
      title="Compare schema"
      icon={<GitCompareArrows size={18} />}
      onClose={onClose}
      footer={footer}
      size="lg"
    >
      <div className="form-grid">
        <label>Source</label>
        <span className="mono">
          {store.connection(ws.connectionId)?.name} · {source.database}
          {ws.caps.hasSchemas ? ` · ${source.schema}` : ""}
        </span>
        <label>Target</label>
        <div className="row wrap">
          <select
            className="select"
            aria-label="Target connection"
            value={target.connectionId}
            onChange={(e) => {
              const w = workspaces[e.target.value];
              if (w)
                setTarget({ connectionId: w.connectionId, database: w.database, schema: w.schema });
              setResult(null);
            }}
          >
            {peers.map((w) => (
              <option key={w.connectionId} value={w.connectionId}>
                {store.connection(w.connectionId)?.name ?? w.connectionId}
              </option>
            ))}
          </select>
          <select
            className="select"
            aria-label="Target database"
            value={target.database}
            onChange={(e) => {
              setTarget({ ...target, database: e.target.value });
              setResult(null);
            }}
          >
            {choices.databases.map((d) => (
              <option key={d}>{d}</option>
            ))}
          </select>
          {choices.schemas.length > 0 && (
            <select
              className="select"
              aria-label="Target schema"
              value={target.schema}
              onChange={(e) => {
                setTarget({ ...target, schema: e.target.value });
                setResult(null);
              }}
            >
              {choices.schemas.map((s) => (
                <option key={s}>{s}</option>
              ))}
            </select>
          )}
        </div>
      </div>
      {same && <div className="faint small">Pick another database or schema to compare with.</div>}
      <ErrorCallout error={error} />
      {result && (
        <>
          <DiffList diff={result.diff} picked={picked} onPicked={setPicked} />
          <Check checked={dropExtra} onChange={setDropExtra}>
            Also drop tables and columns the source doesn't have
          </Check>
          <pre className="sql-preview mono selectable compare-script">
            {script || "-- Nothing to change."}
          </pre>
        </>
      )}
    </Modal>
  );
}
