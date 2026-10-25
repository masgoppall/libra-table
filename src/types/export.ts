// Export jobs (EXP-*): Rust streams rows to the file and reports progress events.

import { type PageRequest } from "./data";
import { type AppError } from "./errors";

export type ExportFormat = "csv" | "json" | "sql";

export type ExportSource =
  { kind: "query"; sql: string } | ({ kind: "table"; pageOnly: boolean } & PageRequest);

export interface ExportOptions {
  delimiter: string;
  quote: string;
  header: boolean;
  bom: boolean;
  nullAs: "empty" | "null";
  tableName: string;
}

export interface ExportRequest {
  source: ExportSource;
  format: ExportFormat;
  path: string;
  options: ExportOptions;
}

export interface ExportProgress {
  jobId: string;
  rows: number;
  done: boolean;
  error: AppError | null;
  path: string;
}
