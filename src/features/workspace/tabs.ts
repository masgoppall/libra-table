// What a tab shows in its icon and tooltip, and the tabs other views open.

import {
  Activity,
  Columns3,
  FileCode2,
  Gauge,
  History,
  type LucideIcon,
  Network,
  ScrollText,
  Table2,
  TablePropertiesIcon,
  Users,
} from "lucide-react";
import { KIND_LABEL, objectTitle } from "@/domain/objects";
import { type NewTab, type TabKind, type Workspace, type WorkspaceTab } from "@/types";

export const TAB_ICONS: Record<TabKind, LucideIcon> = {
  data: Table2,
  structure: Columns3,
  query: FileCode2,
  history: History,
  "new-table": TablePropertiesIcon,
  processes: Activity,
  users: Users,
  definition: ScrollText,
  diagram: Network,
  metrics: Gauge,
};

export function tabTooltip(ws: Workspace, t: WorkspaceTab): string {
  const schema = ws.info.dialect === "postgres" || ws.caps.hasSchemas ? t.schema : undefined;
  const where = [t.database ?? ws.database, schema].filter(Boolean).join(".");
  if (t.kind === "data" || t.kind === "structure") return `${where}.${t.table} · ${t.kind}`;
  if (t.kind === "query") {
    const what = t.savedName ? `Saved query “${t.savedName}”` : "Query";
    return `${what} · ${where}`;
  }
  if (t.kind === "definition" && t.object) {
    return `${where}.${objectTitle(t.object)} · ${KIND_LABEL[t.object.kind]}`;
  }
  return t.title;
}

export const newQueryTab = (sql = "", extra: Partial<NewTab> = {}): NewTab => ({
  kind: "query",
  title: "Query",
  sql,
  ...extra,
});

export const historyTab = (): NewTab => ({ kind: "history", title: "History" });

/** The ER diagram of a schema (one per schema and database). */
export const diagramTab = (schema: string): NewTab => ({
  kind: "diagram",
  title: `Diagram · ${schema}`,
  schema,
});

export const metricsTab = (): NewTab => ({ kind: "metrics", title: "Metrics" });

export const newTableTab = (schema: string): NewTab => ({
  kind: "new-table",
  title: "New table",
  schema,
});

/** The data or structure tab of a table (reused when already open). */
export function tableTab(
  kind: "data" | "structure",
  table: string,
  where: { schema?: string; database?: string },
): NewTab {
  return { kind, title: table, table, schema: where.schema, database: where.database };
}
