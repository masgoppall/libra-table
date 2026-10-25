// The tool and command a dialog's request will run, refreshed as the request changes.

import { useCallback, useState } from "react";
import { useDebouncedEffect } from "@/hooks/useDebouncedEffect";
import { type AppError, type ToolPlan } from "@/types";

const DELAY_MS = 150;

export function useToolPlan<R>(request: R, plan: (r: R) => Promise<ToolPlan>) {
  const [result, setResult] = useState<ToolPlan | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [version, setVersion] = useState(0);

  const load = useCallback(async () => {
    try {
      setResult(await plan(request));
      setError(null);
    } catch (e) {
      setError(e as AppError);
    }
    // `plan` is a fresh closure every render; the request and `version` say when to reload.
  }, [JSON.stringify(request), version]);

  useDebouncedEffect(() => void load(), [load], DELAY_MS);

  /** After the tool path changed. */
  const reload = () => setVersion((v) => v + 1);

  return { plan: result, error, reload };
}
