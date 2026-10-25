// Imports (CSV/JSON, SQL files) and multi-table exports; progress arrives as events.

import {
  type Db,
  type Driver,
  type ExportTablesRequest,
  type FilePreview,
  type ImportRequest,
  type ImportSource,
  type NewColumn,
  type SqlFileRequest,
} from "@/types";
import { call } from "./client";

export const transferApi = {
  previewImportFile: (driver: Driver, source: ImportSource) =>
    call<FilePreview>("preview_import_file", { driver, source }),
  previewImport: (id: string, db: Db, request: ImportRequest, newColumns: NewColumn[] | null) =>
    call<string[]>("preview_import", { id, database: db, request, newColumns }),
  startImport: (id: string, db: Db, jobId: string, request: ImportRequest, confirmed: boolean) =>
    call<void>("start_import", { id, database: db, jobId, request, confirmed }),
  startSqlFile: (id: string, db: Db, jobId: string, request: SqlFileRequest, confirmed: boolean) =>
    call<void>("start_sql_file", { id, database: db, jobId, request, confirmed }),
  cancelImport: (jobId: string) => call<boolean>("cancel_import", { jobId }),
  startExportTables: (id: string, db: Db, jobId: string, request: ExportTablesRequest) =>
    call<void>("start_export_tables", { id, database: db, jobId, request }),
};
