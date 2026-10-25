// A backup or restore job: start it in Rust, follow "tool-progress" events, cancel it.

import { useRef, useState } from "react";
import { api } from "@/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { uid } from "@/lib/uid";
import { type AppError, type ToolProgress } from "@/types";
import { appendLine } from "./backupModel";

export interface ToolJob {
  id: string;
  bytes: number;
  total: number | null;
}

export interface JobHandlers {
  onDone: (p: ToolProgress) => void;
  onCancelled: () => void;
}

export function useToolJob(handlers: JobHandlers) {
  const [job, setJob] = useState<ToolJob | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [error, setError] = useState<AppError | null>(null);
  // Events of other dialogs' jobs are ignored.
  const jobRef = useRef<string | null>(null);

  useTauriEvent<ToolProgress>("tool-progress", (p) => {
    if (p.jobId !== jobRef.current) return;
    if (p.line) setLines((xs) => appendLine(xs, p.line ?? ""));
    if (!p.done) return setJob({ id: p.jobId, bytes: p.bytes, total: p.total });
    jobRef.current = null;
    setJob(null);
    if (p.error?.kind === "cancelled") handlers.onCancelled();
    else if (p.error) setError(p.error);
    else handlers.onDone(p);
  });

  /** Runs `startJob(jobId)`; it resolves false when the user backed out. */
  const start = async (startJob: (jobId: string) => Promise<boolean>) => {
    setError(null);
    setLines([]);
    const id = uid();
    jobRef.current = id;
    setJob({ id, bytes: 0, total: null });
    try {
      if (!(await startJob(id))) {
        jobRef.current = null;
        setJob(null);
      }
    } catch (e) {
      jobRef.current = null;
      setJob(null);
      setError(e as AppError);
    }
  };

  const cancel = () => job && void api.cancelToolJob(job.id);

  return { job, lines, error, setError, start, cancel };
}
