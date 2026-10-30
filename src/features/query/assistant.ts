// What the SQL assistant is told about the database: "table: col, col" lines, within a
// size the request can carry. When the schema is larger, the answer says how much was sent.

import { type CompletionSchema } from "@/types";

const MAX_CHARS = 80_000;

export interface SchemaSummary {
  text: string;
  sent: number;
  total: number;
}

export function schemaSummary(c: CompletionSchema | null, max = MAX_CHARS): SchemaSummary {
  const entries = Object.entries(c?.tables ?? {}).sort(([a], [b]) => a.localeCompare(b));
  const lines: string[] = [];
  let size = 0;
  for (const [table, columns] of entries) {
    const line = `${table}: ${columns.join(", ")}`;
    if (size + line.length + 1 > max) break;
    lines.push(line);
    size += line.length + 1;
  }
  return { text: lines.join("\n"), sent: lines.length, total: entries.length };
}
