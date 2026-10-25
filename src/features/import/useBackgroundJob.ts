// A background job in Rust that reports `JobEvent`s: progress, then a summary or an error.

import { useRef, useState } from "react";
import { api } from "@/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { uid } from "@/lib/uid";
import { type AppError, type JobEvent } from "@/types";

export interface BackgroundJob<P, S> {
  running: boolean;
  progress: P | null;
  summary: S | null;
  error: AppError | null;
  /** Starts a job: `run(jobId)` asks Rust to start it; false means it didn't start. */
  start: (run: (jobId: string) => Promise<boolean>) => Promise<void>;
  cancel: () => void;
  reset: () => void;
}

export function useBackgroundJob<P, S>(event: string, onDone?: () => void): BackgroundJob<P, S> {
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<P | null>(null);
  const [summary, setSummary] = useState<S | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  // Events of other jobs are ignored.
  const current = useRef<string | null>(null);

  const finish = () => {
    current.current = null;
    setJobId(null);
  };

  useTauriEvent<JobEvent<P, S>>(event, (e) => {
    if (e.jobId !== current.current) return;
    if (!e.done) return setProgress(e.progress);
    finish();
    setSummary(e.summary);
    setError(e.error);
    onDone?.();
  });

  const start = async (run: (jobId: string) => Promise<boolean>) => {
    const id = uid();
    current.current = id;
    setJobId(id);
    setProgress(null);
    setSummary(null);
    setError(null);
    try {
      if (!(await run(id))) finish();
    } catch (e) {
      finish();
      setError(e as AppError);
    }
  };

  const reset = () => {
    finish();
    setProgress(null);
    setSummary(null);
    setError(null);
  };

  const cancel = () => jobId && void api.cancelImport(jobId);

  return { running: jobId !== null, progress, summary, error, start, cancel, reset };
}
