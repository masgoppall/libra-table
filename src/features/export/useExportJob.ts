// An export job: pick the file, start it in Rust, follow "export-progress" events.

import { save } from "@tauri-apps/plugin-dialog";
import { useRef, useState } from "react";
import { api } from "@/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { uid } from "@/lib/uid";
import { plural } from "@/lib/format";
import { toastInfo, toastSuccess } from "@/services/errors";
import { withSafeMode } from "@/services/safeMode";
import { store } from "@/store";
import { type AppError, type Db, type ExportProgress, type ExportRequest } from "@/types";

export interface ExportJob {
  id: string;
  rows: number;
}

export function useExportJob(connectionId: string, db: Db, onDone: () => void) {
  const [job, setJob] = useState<ExportJob | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  // Events of other dialogs' jobs are ignored.
  const jobRef = useRef<string | null>(null);

  const finish = () => {
    jobRef.current = null;
    setJob(null);
  };

  useTauriEvent<ExportProgress>("export-progress", (p) => {
    if (p.jobId !== jobRef.current) return;
    if (!p.done) return setJob({ id: p.jobId, rows: p.rows });
    finish();
    if (p.error?.kind === "cancelled") toastInfo("Export cancelled.");
    else if (p.error) setError(p.error);
    else {
      toastSuccess(`Exported ${plural(p.rows, "row")}.`, p.path);
      onDone();
    }
  });

  const start = async (defaultName: string, ext: string, req: Omit<ExportRequest, "path">) => {
    setError(null);
    const path = await save({
      defaultPath: `${defaultName || "export"}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    const id = uid();
    jobRef.current = id;
    setJob({ id, rows: 0 });
    try {
      const name = store.connection(connectionId)?.name ?? "";
      const started = await withSafeMode(name, (confirmed) =>
        api.startExport(connectionId, db, id, { ...req, path }, confirmed).then(() => true),
      );
      if (!started) finish();
    } catch (e) {
      finish();
      setError(e as AppError);
    }
  };

  const cancel = () => job && void api.cancelExport(job.id);

  return { job, error, start, cancel };
}
