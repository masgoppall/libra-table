// State of a structure / new-table tab: load, edit, validate, apply.

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/api";
import { confirmDiscard } from "@/services/confirm";
import { runSchemaChange } from "@/services/schemaChange";
import { store, tabDb } from "@/store";
import { type AppError, type TableStructure, type Workspace, type WorkspaceTab } from "@/types";
import * as model from "./designerModel";

export type Section = "columns" | "indexes" | "foreign" | "ddl";

function useTableNames(ws: Workspace, schema: string) {
  const [tables, setTables] = useState<string[]>([]);
  const db = tabDb(ws);
  useEffect(() => {
    api
      .listObjects(ws.connectionId, db, schema)
      .then((o) => setTables(o.filter((x) => x.kind === "table").map((x) => x.name)))
      .catch(() => setTables([]));
  }, [ws.connectionId, db, schema, ws.refreshKey]);
  return tables;
}

export function useTableDesigner(ws: Workspace, tab: WorkspaceTab) {
  const id = ws.connectionId;
  const driver = ws.info.driver;
  const db = tabDb(ws, tab);
  const schema = tab.schema ?? ws.schema;
  const creating = tab.kind === "new-table";
  const [structure, setStructure] = useState<TableStructure | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [section, setSection] = useState<Section>("columns");
  const [name, setName] = useState(creating ? "" : (tab.table ?? ""));
  const [comment, setComment] = useState("");
  const [cols, setCols] = useState<model.ColRow[]>(() =>
    creating ? [model.newColumnRow(driver, true)] : [],
  );
  const [idx, setIdx] = useState<model.IdxRow[]>([]);
  const [fks, setFks] = useState<model.FkRow[]>([]);
  const tables = useTableNames(ws, schema);

  const load = useCallback(async () => {
    if (creating || !tab.table) return;
    setLoading(true);
    setError(null);
    try {
      await api.refreshMetadata(id).catch(() => undefined);
      const s = await api.tableStructure(id, db, schema, tab.table);
      const rows = model.rowsFromStructure(s);
      setStructure(s);
      setCols(rows.cols);
      setIdx(rows.idx);
      setFks(rows.fks);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setLoading(false);
    }
  }, [id, db, schema, tab.table, creating]);

  useEffect(() => {
    void load();
  }, [load, ws.refreshKey]);

  const state: model.DesignerState = {
    creating,
    schema,
    name: creating ? name : (tab.table ?? ""),
    comment,
    cols,
    idx,
    fks,
  };
  const change = useMemo(() => model.buildChange(state), [cols, idx, fks, name, comment, schema]);
  const problems = useMemo(() => model.validate(state), [cols, idx, fks, name, creating]);
  const pending = change?.kind === "alter_table" ? change.ops.length : 0;

  const discard = () => {
    if (!creating) return void load();
    setCols([model.newColumnRow(driver, true)]);
    setIdx([]);
    setFks([]);
  };

  const reload = async () => {
    const wording = { doing: "Reloading", verb: "reload", kind: "unsaved" };
    if (await confirmDiscard(pending, wording)) void load();
  };

  const apply = async () => {
    if (!change || problems.length || busy) return;
    const table = state.name.trim();
    setBusy(true);
    const ok = await runSchemaChange(id, db, change, {
      title: creating ? `Create table ${table}` : `Change ${table}`,
      okLabel: creating ? "Create table" : "Apply changes",
      success: creating ? `Created table "${table}".` : `Updated "${table}".`,
    });
    setBusy(false);
    if (!ok) return;
    // The new-table tab becomes the new table's structure.
    if (creating)
      store.get().updateTab(id, tab.id, { kind: "structure", table, title: table, schema });
    store.get().refresh(id);
  };

  const addColumn = () => {
    const row = model.newColumnRow(driver, false);
    setCols((rows) => [...rows, row]);
    setSection("columns");
    setTimeout(() => {
      document.querySelector<HTMLInputElement>(`[data-col="${row.key}"] input`)?.focus();
    }, 0);
  };

  return {
    connectionId: id,
    driver,
    schema,
    db,
    creating,
    structure,
    isView: structure?.kind === "view",
    error,
    loading,
    busy,
    section,
    setSection,
    name,
    setName,
    comment,
    setComment,
    cols,
    setCols,
    idx,
    setIdx,
    fks,
    setFks,
    tables,
    change,
    problems,
    pending,
    load,
    reload,
    discard,
    apply,
    addColumn,
  };
}

export type TableDesignerState = ReturnType<typeof useTableDesigner>;
