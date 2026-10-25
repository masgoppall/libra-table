// Loads an object's definition (and a sequence's current settings) for its tab.

import { useCallback, useEffect, useState } from "react";
import { api } from "@/api";
import { tabDb } from "@/store";
import {
  type AppError,
  type ObjectDefinition,
  type SchemaObjectRef,
  type SequenceInfo,
  type Workspace,
  type WorkspaceTab,
} from "@/types";

export function useDefinition(ws: Workspace, tab: WorkspaceTab, object: SchemaObjectRef) {
  const id = ws.connectionId;
  const db = tabDb(ws, tab);
  const schema = tab.schema ?? ws.schema;
  const [definition, setDefinition] = useState<ObjectDefinition | null>(null);
  const [sequence, setSequence] = useState<SequenceInfo | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const isSequence = object.kind === "sequence";
      const [def, seq] = await Promise.all([
        api.objectDefinition(id, db, schema, object),
        isSequence ? api.sequenceInfo(id, db, schema, object.name) : Promise.resolve(null),
      ]);
      setDefinition(def);
      setSequence(seq);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setLoading(false);
    }
  }, [id, db, schema, object.kind, object.name, object.arguments, object.table]);

  useEffect(() => {
    void load();
  }, [load, ws.refreshKey]);

  return { schema, definition, sequence, error, loading, reload: load };
}

export type DefinitionState = ReturnType<typeof useDefinition>;
