// pg_dump / mysqldump options of the backup dialog.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { defaultSchema } from "@/domain/schema";
import { type BackupRequest, type Db, type DumpContent, type Workspace } from "@/types";
import { toggleName } from "./backupModel";

type Set = (patch: Partial<BackupRequest>) => void;

const CONTENT: [DumpContent, string][] = [
  ["all", "Structure and data"],
  ["schema_only", "Structure only"],
  ["data_only", "Data only"],
];

/** Tables of the database's default schema (Postgres) or of the database (MySQL). */
function useTables(ws: Workspace, db: Db, schema: string) {
  const [tables, setTables] = useState<string[]>([]);
  useEffect(() => {
    api.listObjects(ws.connectionId, db, schema).then(
      (objs) => setTables(objs.filter((o) => o.kind === "table").map((o) => o.name)),
      () => setTables([]),
    );
  }, [ws.connectionId, db, schema]);
  return tables;
}

function useSchemas(ws: Workspace, db: Db, enabled: boolean) {
  const [schemas, setSchemas] = useState<string[]>([]);
  useEffect(() => {
    if (!enabled) return;
    api.listSchemas(ws.connectionId, db).then(setSchemas, () => setSchemas([]));
  }, [ws.connectionId, db, enabled]);
  return schemas;
}

function SchemaPicker(props: { schemas: string[]; r: BackupRequest; set: Set }) {
  const toggle = (name: string, on: boolean) =>
    props.set({ schemas: toggleName(props.r.schemas, name, on) });
  return (
    <div className="check-list scroll">
      {props.schemas.map((s) => (
        <Check key={s} checked={props.r.schemas.includes(s)} onChange={(on) => toggle(s, on)}>
          <span className="mono">{s}</span>
        </Check>
      ))}
    </div>
  );
}

function TablePicker(props: { tables: string[]; schema: string; r: BackupRequest; set: Set }) {
  const { r, schema } = props;
  const picked = r.tables.filter((t) => t.schema === schema).map((t) => t.name);
  const toggle = (name: string, on: boolean) =>
    props.set({ tables: toggleName(picked, name, on).map((n) => ({ schema, name: n })) });
  if (props.tables.length === 0) return <span className="faint">No tables.</span>;
  return (
    <div className="check-list scroll">
      {props.tables.map((t) => (
        <Check key={t} checked={picked.includes(t)} onChange={(on) => toggle(t, on)}>
          <span className="mono">{t}</span>
        </Check>
      ))}
    </div>
  );
}

function Flags(props: { items: [keyof BackupRequest, string][]; r: BackupRequest; set: Set }) {
  return (
    <div className="check-list">
      {props.items.map(([key, label]) => (
        <Check
          key={key}
          checked={props.r[key] === true}
          onChange={(on) => props.set({ [key]: on } as Partial<BackupRequest>)}
        >
          {label}
        </Check>
      ))}
    </div>
  );
}

const PG_FLAGS: [keyof BackupRequest, string][] = [
  ["noOwner", "Don't set owners (--no-owner)"],
  ["noPrivileges", "No GRANT/REVOKE (--no-privileges)"],
  ["clean", "Drop objects before creating them (--clean)"],
  ["createDatabase", "Include CREATE DATABASE (--create)"],
];

const MYSQL_FLAGS: [keyof BackupRequest, string][] = [
  ["singleTransaction", "Consistent snapshot (--single-transaction)"],
  ["routines", "Procedures and functions (--routines)"],
  ["triggers", "Triggers"],
  ["events", "Events (--events)"],
  ["createDatabase", "Include CREATE DATABASE and USE (--databases)"],
];

export function BackupOptions(props: {
  ws: Workspace;
  db: Db;
  database: string;
  r: BackupRequest;
  set: Set;
}) {
  const { ws, r, set } = props;
  const pg = ws.info.driver === "postgres";
  const here = props.database === ws.database;
  const pgSchema = here ? ws.schema : defaultSchema("postgres", props.database);
  const schema = pg ? pgSchema : props.database;
  const tables = useTables(ws, props.db, schema);
  const schemas = useSchemas(ws, props.db, pg);
  return (
    <>
      {pg && (
        <>
          <label>Format</label>
          <select
            className="select"
            aria-label="Format"
            value={r.format}
            onChange={(e) => set({ format: e.target.value as BackupRequest["format"] })}
          >
            <option value="plain">Plain SQL (psql)</option>
            <option value="custom">Custom archive (pg_restore)</option>
          </select>
        </>
      )}
      <label>Content</label>
      <select
        className="select"
        aria-label="Content"
        value={r.content}
        onChange={(e) => set({ content: e.target.value as DumpContent })}
      >
        {CONTENT.map(([v, label]) => (
          <option key={v} value={v}>
            {label}
          </option>
        ))}
      </select>
      {pg && (
        <>
          <label>Only schemas</label>
          <SchemaPicker schemas={schemas} r={r} set={set} />
        </>
      )}
      <label>Only tables{pg ? ` in ${schema}` : ""}</label>
      <TablePicker tables={tables} schema={schema} r={r} set={set} />
      <label>Options</label>
      <Flags items={pg ? PG_FLAGS : MYSQL_FLAGS} r={r} set={set} />
    </>
  );
}
