// State of a structure / new-table tab: load, edit, validate, apply.

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/api";
import { confirmDiscard } from "@/services/confirm";
import { runSchemaChange } from "@/services/schemaChange";
import { store, tabDb } from "@/store";
import { type AppError, type TableStructure, type Workspace, type WorkspaceTab } from "@/types";
import * as change from "./designerChange";
import * as model from "./designerModel";

/** What the body shows: the columns, or the indexes (the bottom bar's "Index"). */
export type Section = "columns" | "indexes";

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
  const driver = ws.info.dialect;
  const db = tabDb(ws, tab);
  const schema = tab.schema ?? ws.schema;
  const creating = tab.kind === "new-table";
  const table = creating ? "" : (tab.table ?? "");
  const [structure, setStructure] = useState<TableStructure | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [section, setSection] = useState<Section>("columns");
  const [name, setName] = useState(table);
  const [comment, setComment] = useState("");
  const [cols, setCols] = useState<model.ColRow[]>(() =>
    creating ? [model.newColumnRow(driver, true)] : [],
  );
  const [idx, setIdx] = useState<model.IdxRow[]>([]);
  const [fks, setFks] = useState<model.FkRow[]>([]);
  const tables = useTableNames(ws, schema);

  const load = useCallback(async () => {
    if (creating || !table) return;
    setLoading(true);
    setError(null);
    try {
      await api.refreshMetadata(id).catch(() => undefined);
      const s = await api.tableStructure(id, db, schema, table);
      const rows = model.rowsFromStructure(s);
      setStructure(s);
      setName(table);
      setCols(rows.cols);
      setIdx(rows.idx);
      setFks(rows.fks);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setLoading(false);
    }
  }, [id, db, schema, table, creating]);

  useEffect(() => {
    void load();
  }, [load, ws.refreshKey]);

  const state: model.DesignerState = {
    creating,
    driver,
    schema,
    table,
    name,
    comment,
    cols,
    idx,
    fks,
  };
  const pendingChange = useMemo(
    () => change.buildChange(state),
    [cols, idx, fks, name, comment, schema, table, driver, creating],
  );
  const problems = useMemo(
    () => change.validate(state),
    [cols, idx, fks, name, comment, schema, table, driver, creating],
  );
  const pending = change.pendingCount(pendingChange);

  // The toolbar's discard / commit buttons follow this tab's changes (a new table is
  // always waiting to be created).
  const dirty = creating ? 1 : pending;
  useEffect(() => store.get().setPendingEdits(tab.id, dirty), [tab.id, dirty]);
  useEffect(() => () => store.get().setPendingEdits(tab.id, 0), [tab.id]);

  const discard = () => {
    if (!creating) return void load();
    setName("");
    setCols([model.newColumnRow(driver, true)]);
    setIdx([]);
    setFks([]);
  };

  const reload = async () => {
    const wording = { doing: "Reloading", verb: "reload", kind: "unsaved" };
    if (await confirmDiscard(pending, wording)) void load();
  };

  /** Shows what stops the change, or previews and applies it. */
  const apply = async () => {
    if (busy) return;
    if (problems.length) {
      const [first, ...rest] = problems;
      await store.get().dialog({
        kind: "alert",
        tone: "warning",
        title: creating ? "The table can't be created yet" : "The changes can't be applied yet",
        message: first ?? "",
        details: rest.length ? rest : undefined,
      });
      return;
    }
    if (!pendingChange) return;
    const target = name.trim();
    setBusy(true);
    const ok = await runSchemaChange(id, db, pendingChange, {
      title: creating ? `Create table ${target}` : `Change ${table}`,
      okLabel: creating ? "Create table" : "Apply changes",
      success: creating ? `Created table "${target}".` : `Updated "${target}".`,
    });
    setBusy(false);
    if (!ok) return;
    // The new-table tab becomes the new table's structure; a rename renames the tab.
    if (creating || target !== table) {
      store
        .get()
        .updateTab(id, tab.id, { kind: "structure", table: target, title: target, schema });
    }
    store.get().refresh(id);
  };

  const addColumn = () => {
    const row = model.newColumnRow(driver, false);
    setCols((rows) => [...rows, row]);
    setSection("columns");
    return row.key;
  };

  const addIndex = () => {
    const row = model.newIndexRow();
    setIdx((rows) => [...rows, row]);
    setSection("indexes");
    return row.key;
  };

  return {
    connectionId: id,
    driver,
    schema,
    db,
    table,
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
    change: pendingChange,
    problems,
    pending,
    load,
    reload,
    discard,
    apply,
    addColumn,
    addIndex,
  };
}

export type TableDesignerState = ReturnType<typeof useTableDesigner>;
