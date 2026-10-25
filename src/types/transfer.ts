// Imports (CSV/JSON into a table, SQL files) and exports of several tables. Rust streams
// the files and reports progress events (import-progress, sql-file-progress,
// export-tables-progress).

import { type AppError } from "./errors";
import { type ExportFormat, type ExportOptions } from "./export";

export type ImportFormat = "csv" | "json";
export type ImportEncoding = "utf8" | "latin1";

export interface CsvOptions {
  /** One character; "\t" for tab. */
  delimiter: string;
  /** One character, or "" for no quoting. */
  quote: string;
  header: boolean;
  encoding: ImportEncoding;
  /** A field equal to this is NULL ("" for none). */
  nullMarker: string;
  emptyAsNull: boolean;
  /** Rows skipped at the start of the file, before the header. */
  skipRows: number;
}

export interface ImportSource {
  path: string;
  format: ImportFormat;
  csv: CsvOptions;
}

export type InferredKind =
  | "integer"
  | "big_int"
  | "decimal"
  | "double"
  | "boolean"
  | "date"
  | "timestamp"
  | "timestamp_tz"
  | "json"
  | "text";

export interface SourceColumn {
  name: string;
  kind: InferredKind;
  /** A type for the connection's driver, for new tables. */
  suggestedType: string;
  nullable: boolean;
}

export interface RowError {
  /** Data row, counted from 1. */
  row: number;
  /** File line (CSV). */
  line: number | null;
  message: string;
}

export interface FilePreview {
  columns: SourceColumn[];
  rows: (string | null)[][];
  sampled: number;
  totalBytes: number;
  errors: RowError[];
}

/** Source column index → target column. */
export interface ColumnMap {
  source: number;
  target: string;
}

export type ImportOnError = "stop" | "skip";
export type ImportTransaction = "run" | "batch";

export interface ImportRequest {
  source: ImportSource;
  sourceColumns: string[];
  schema: string;
  table: string;
  mapping: ColumnMap[];
  batchSize: number;
  onError: ImportOnError;
  transaction: ImportTransaction;
}

/** A column of a table created for the import (before it exists). */
export interface NewColumn {
  name: string;
  dataType: string;
}

export interface ImportProgress {
  rowsRead: number;
  inserted: number;
  skipped: number;
  bytesRead: number;
  totalBytes: number;
}

export interface ImportSummary {
  inserted: number;
  skipped: number;
  rowsRead: number;
  errors: RowError[];
  errorCount: number;
  cancelled: boolean;
  stopped: boolean;
  rolledBack: boolean;
  fatal: AppError | null;
}

/** A background job's event: progress until `done`, then a summary or an error. */
export interface JobEvent<P, S> {
  jobId: string;
  progress: P;
  done: boolean;
  summary: S | null;
  error: AppError | null;
}

export interface SqlFileRequest {
  path: string;
  stopOnError: boolean;
}

export interface SqlFileProgress {
  statements: number;
  failed: number;
  bytesRead: number;
  totalBytes: number;
}

export interface StatementError {
  index: number;
  line: number;
  sql: string;
  message: string;
}

export interface SqlFileSummary {
  statements: number;
  failed: number;
  rowsAffected: number;
  errors: StatementError[];
  skippedCommands: number;
  cancelled: boolean;
  stopped: boolean;
  fatal: AppError | null;
}

export interface ExportTablesRequest {
  schema: string;
  tables: string[];
  format: ExportFormat;
  folder: string;
  options: ExportOptions;
}

export interface ExportTablesProgress {
  /** 0-based index of the table being written. */
  index: number;
  count: number;
  table: string;
  rows: number;
  totalRows: number;
  files: string[];
}

export interface ExportTablesEvent {
  jobId: string;
  progress: ExportTablesProgress;
  done: boolean;
  error: AppError | null;
}
