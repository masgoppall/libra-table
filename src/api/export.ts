// Export jobs; progress arrives as "export-progress" events.

import { type Db, type ExportRequest } from "@/types";
import { call } from "./client";

export const exportApi = {
  startExport: (id: string, db: Db, jobId: string, request: ExportRequest, confirmed: boolean) =>
    call<void>("start_export", { id, database: db, jobId, request, confirmed }),
  cancelExport: (jobId: string) => call<boolean>("cancel_export", { jobId }),
  /** Deletes the oldest `<prefix>-<date>` backups in `folder`, keeping `keep`. */
  pruneBackups: (folder: string, prefix: string, keep: number) =>
    call<number>("prune_backups", { folder, prefix, keep }),
  saveTextFile: (path: string, contents: string) =>
    call<void>("save_text_file", { path, contents }),
};
