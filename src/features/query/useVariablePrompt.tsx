// The variables dialog as a promise: `await prompt(vars)` gives the values or null.

import { useState } from "react";
import { type QueryVariable } from "@/types";
import { VariablesDialog } from "./VariablesDialog";

interface Pending {
  vars: QueryVariable[];
  resolve: (v: QueryVariable[] | null) => void;
}

export function useVariablePrompt() {
  const [pending, setPending] = useState<Pending | null>(null);
  const prompt = (vars: QueryVariable[]) =>
    new Promise<QueryVariable[] | null>((resolve) => setPending({ vars, resolve }));
  const node = pending && (
    <VariablesDialog
      initial={pending.vars}
      onDone={(v) => {
        setPending(null);
        pending.resolve(v);
      }}
    />
  );
  return { prompt, node };
}
