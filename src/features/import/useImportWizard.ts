// State of the import wizard: the file and its options, the preview Rust reads, the
// column mapping (or the new table), the run options, and the job.

import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api } from "@/api";
import { store, type ToolDialog } from "@/store";
import {
  type AppError,
  type ColumnInfo,
  type CsvOptions,
  type Dialect,
  type FilePreview,
  type ImportProgress,
  type ImportSource,
  type ImportSummary,
} from "@/types";
import { startExisting, startNewTable } from "./importFlow";
import * as model from "./importModel";
import { useBackgroundJob } from "./useBackgroundJob";

export type ImportStep = "source" | "columns" | "run";
type ImportDialog = Extract<ToolDialog, { kind: "import" }>;

const FILE_FILTERS = [
  { name: "CSV or JSON", extensions: ["csv", "tsv", "txt", "json", "ndjson", "jsonl"] },
];

/** The file preview, read again whenever the file or its options change. */
function usePreview(driver: Dialect, source: ImportSource) {
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  // Options change while typing: wait a moment before reading the file again.
  useEffect(() => {
    if (!source.path) return;
    let live = true;
    const t = setTimeout(() => {
      setLoading(true);
      api
        .previewImportFile(driver, source)
        .then((p) => live && (setPreview(p), setError(null)))
        .catch((e) => live && (setPreview(null), setError(e as AppError)))
        .finally(() => live && setLoading(false));
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [source.path, source.format, JSON.stringify(source.csv)]);
  return { preview, error, loading };
}

/** Columns of the existing target table. */
function useTargetColumns(d: ImportDialog) {
  const [columns, setColumns] = useState<ColumnInfo[]>([]);
  useEffect(() => {
    if (!d.table) return;
    api
      .listColumns(d.connectionId, d.db, d.schema, d.table)
      .then(setColumns)
      .catch(() => setColumns([]));
  }, [d.connectionId, d.table]);
  return columns;
}

export function useImportWizard(d: ImportDialog, driver: Dialect) {
  const [step, setStep] = useState<ImportStep>("source");
  const [source, setSource] = useState<ImportSource>({
    path: "",
    format: "csv",
    csv: model.DEFAULT_CSV,
  });
  const { preview, error: previewError, loading } = usePreview(driver, source);
  const targetColumns = useTargetColumns(d);
  const [targets, setTargets] = useState<(string | null)[]>([]);
  const [tableName, setTableName] = useState("");
  const [newColumns, setNewColumns] = useState<model.NewTableColumn[]>([]);
  const [options, setOptions] = useState(model.DEFAULT_OPTIONS);
  const job = useBackgroundJob<ImportProgress, ImportSummary>("import-progress", () =>
    store.get().refresh(d.connectionId),
  );

  // A new set of source columns starts the mapping over.
  const names = preview?.columns.map((c) => c.name).join("\n") ?? "";
  useEffect(() => {
    if (!preview) return;
    const targetNames = targetColumns.map((c) => c.name);
    setTargets(
      model.autoMatch(
        preview.columns.map((c) => c.name),
        targetNames,
      ),
    );
    setNewColumns(model.newTableColumns(preview));
  }, [names, targetColumns]);

  const chooseFile = async () => {
    const path = await open({ multiple: false, filters: FILE_FILTERS });
    if (typeof path !== "string") return;
    setSource((s) => ({ ...s, path, format: model.formatOf(path) }));
    if (!tableName) setTableName(model.tableNameFrom(path));
  };

  const setCsv = (patch: Partial<CsvOptions>) =>
    setSource((s) => ({ ...s, csv: { ...s.csv, ...patch } }));

  const isNew = d.table === null;
  const finalTargets = isNew ? model.newTableTargets(newColumns) : targets;
  const problem = isNew
    ? model.newTableProblem(tableName, newColumns)
    : model.mappingProblem(targets);

  const start = () => {
    if (!preview) return;
    const where = { schema: d.schema, table: isNew ? tableName.trim() : (d.table ?? "") };
    const req = model.buildRequest(source, preview, where, finalTargets, options);
    const t = { connectionId: d.connectionId, db: d.db };
    setStep("run");
    void job.start((jobId) => {
      if (!isNew) return startExisting(t, jobId, req);
      const change = model.createTableChange(d.schema, tableName, newColumns);
      return startNewTable(t, jobId, change, req, model.newTableSpec(newColumns));
    });
  };

  return {
    step,
    setStep,
    source,
    setFormat: (format: ImportSource["format"]) => setSource((s) => ({ ...s, format })),
    setCsv,
    chooseFile,
    preview,
    previewError,
    loading,
    isNew,
    targetColumns,
    targets,
    setTargets,
    tableName,
    setTableName,
    newColumns,
    setNewColumns,
    options,
    setOptions,
    problem,
    start,
    job,
  };
}

export type ImportWizardState = ReturnType<typeof useImportWizard>;
