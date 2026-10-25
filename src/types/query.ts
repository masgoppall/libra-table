// SQL editor: statements, results, saved queries and history.

import { type AppError } from "./errors";
import { type Cell } from "./data";

export interface Statement {
  text: string;
  from: number;
  to: number;
}

export interface ColumnMeta {
  name: string;
  typeName: string;
}

export interface StatementResult {
  sql: string;
  columns: ColumnMeta[];
  rows: Cell[][];
  totalRows: number;
  truncated: boolean;
  rowsAffected: number | null;
  durationMs: number;
  error: AppError | null;
}

export interface RunResponse {
  results: StatementResult[];
  cancelled: boolean;
}

export type HistoryStatus = "ok" | "error" | "cancelled";

export interface HistoryEntry {
  id: number;
  connectionId: string;
  connectionName: string;
  database: string;
  sql: string;
  startedAt: number;
  durationMs: number;
  rowCount: number | null;
  status: HistoryStatus;
  error: string | null;
}

export interface HistoryQuery {
  text?: string;
  connectionId?: string | null;
  from?: number | null;
  to?: number | null;
  limit?: number;
  offset?: number;
}

export interface SavedQuery {
  name: string;
  modifiedAt: number;
  size: number;
}
