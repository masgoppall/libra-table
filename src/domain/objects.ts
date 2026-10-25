// Schema objects (functions, triggers…): names for messages and identity rules.

import { type SchemaObject, type SchemaObjectKind, type SchemaObjectRef } from "@/types";

export const KIND_LABEL: Record<SchemaObjectKind, string> = {
  function: "function",
  procedure: "procedure",
  sequence: "sequence",
  materialized_view: "materialized view",
  trigger: "trigger",
  type: "type",
  event: "event",
};

export function objectRef(o: SchemaObject | SchemaObjectRef): SchemaObjectRef {
  return { kind: o.kind, name: o.name, arguments: o.arguments ?? null, table: o.table ?? null };
}

/** "add(a integer)" for routines, "audit on people" for triggers, else the name. */
export function objectTitle(o: SchemaObjectRef): string {
  if (o.kind === "function" || o.kind === "procedure") return `${o.name}(${o.arguments ?? ""})`;
  if (o.kind === "trigger" && o.table) return `${o.name} on ${o.table}`;
  return o.name;
}

/** Same object: overloads (arguments) and triggers (table) of one name differ. */
export function sameObject(a?: SchemaObjectRef, b?: SchemaObjectRef): boolean {
  if (!a || !b) return a === b;
  const same = (x?: string | null, y?: string | null) => (x ?? "") === (y ?? "");
  return (
    a.kind === b.kind &&
    a.name === b.name &&
    same(a.arguments, b.arguments) &&
    same(a.table, b.table)
  );
}
