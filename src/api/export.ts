// Export jobs; progress arrives as "export-progress" events.

import { type Db, type ExportRequest } from "@/types";
import { call } from "./client";

export const exportApi = {
  startExport: (id: string, db: Db, jobId: string, request: ExportRequest, confirmed: boolean) =>
    call<void>("start_export", { id, database: db, jobId, request, confirmed }),
  cancelExport: (jobId: string) => call<boolean>("cancel_export", { jobId }),
};
