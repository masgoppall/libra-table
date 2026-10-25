// Context menus of the sidebar: tables and views, databases, saved queries, schemas.

import {
  ArchiveRestore,
  Columns3,
  Copy,
  CopyPlus,
  Database,
  DatabaseBackup,
  Download,
  Eraser,
  FileCode2,
  FileUp,
  FolderPlus,
  Network,
  Pencil,
  Plus,
  SquareTerminal,
  Table2,
  Terminal,
  Trash2,
} from "lucide-react";
import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { defaultSchema } from "@/domain/schema";
import { copyText } from "@/lib/clipboard";
import { selectAllSql } from "@/lib/sql";
import { toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type ObjectInfo, type SavedQuery, type Workspace } from "@/types";
import * as tools from "../import/openTools";
import { deleteSavedQuery, openSavedQuery, renameSavedQuery } from "../query/savedQueries";
import * as flows from "../schema/flows";
import { openServerDialog } from "../server/serverActions";
import { newQueryTab, tableTab } from "../workspace/tabs";

const icon = (Icon: typeof Copy) => <Icon size={14} />;

async function copy(text: string, what: string) {
  await copyText(text);
  toastSuccess(`${what} copied.`);
}

function tableItems(ws: Workspace, name: string): MenuItem[] {
  const schema = ws.schema;
  return [
    { label: "Rename…", icon: icon(Pencil), onClick: () => flows.renameTable(ws, schema, name) },
    {
      label: "Duplicate…",
      icon: icon(CopyPlus),
      onClick: () => flows.duplicateTable(ws, schema, name),
    },
    {
      label: "Empty table…",
      icon: icon(Eraser),
      danger: true,
      onClick: () => flows.truncateTable(ws, schema, name),
    },
    {
      label: "Drop table…",
      icon: icon(Trash2),
      danger: true,
      onClick: () => flows.dropTable(ws, schema, name, "table"),
    },
  ];
}

export function objectMenu(
  ws: Workspace,
  o: ObjectInfo,
  onExport: (schema: string, table: string) => void,
  /** More items (pin / unpin), placed before the destructive ones. */
  extra: MenuItem[] = [],
): MenuItem[] {
  const id = ws.connectionId;
  const { addTab } = store.get();
  const isTable = o.kind === "table";
  const select = selectAllSql(ws.info.driver, ws.schema, o.name);
  const where = { schema: ws.schema };
  const dropView: MenuItem = {
    label: "Drop view…",
    icon: icon(Trash2),
    danger: true,
    onClick: () => flows.dropTable(ws, ws.schema, o.name, "view"),
  };
  return [
    {
      label: "Open data",
      icon: icon(Table2),
      onClick: () => addTab(id, tableTab("data", o.name, where), { reuse: true }),
    },
    {
      label: isTable ? "Edit structure" : "Show definition",
      icon: icon(Columns3),
      onClick: () => addTab(id, tableTab("structure", o.name, where), { reuse: true }),
    },
    {
      label: "New query with SELECT",
      icon: icon(SquareTerminal),
      onClick: () => addTab(id, newQueryTab(`${select}\n`, { title: `Query · ${o.name}` })),
    },
    SEPARATOR,
    { label: "Copy name", icon: icon(Copy), onClick: () => void copy(o.name, "Name") },
    { label: "Copy SELECT … LIMIT 100", onClick: () => void copy(select, "SELECT") },
    { label: "Export…", icon: icon(Download), onClick: () => onExport(ws.schema, o.name) },
    ...(isTable
      ? [
          { label: "Import…", icon: icon(FileUp), onClick: () => tools.openImport(ws, o.name) },
          { label: "Export tables…", onClick: () => tools.openExportTables(ws, [o.name]) },
        ]
      : []),
    ...extra,
    SEPARATOR,
    ...(isTable ? tableItems(ws, o.name) : [dropView]),
  ];
}

/** Backup, restore and the command line for one database. */
function serverItems(ws: Workspace, name: string): MenuItem[] {
  const open = (kind: "backup" | "restore" | "command-line") => () =>
    openServerDialog(ws, kind, name);
  return [
    { label: "Backup…", icon: icon(DatabaseBackup), onClick: open("backup") },
    { label: "Restore…", icon: icon(ArchiveRestore), onClick: open("restore") },
    { label: "Open in command line", icon: icon(Terminal), onClick: open("command-line") },
  ];
}

export function databaseMenu(ws: Workspace, name: string): MenuItem[] {
  const id = ws.connectionId;
  const { addTab, switchDatabase } = store.get();
  const query = newQueryTab("", {
    title: `Query · ${name}`,
    database: name,
    schema: defaultSchema(ws.info.driver, name),
  });
  return [
    { label: "Open", icon: icon(Database), onClick: () => switchDatabase(id, name) },
    {
      label: "New query in this database",
      icon: icon(SquareTerminal),
      onClick: () => addTab(id, query),
    },
    {
      label: "Run SQL file…",
      icon: icon(FileCode2),
      onClick: () => tools.openSqlFile(ws, name),
    },
    { label: "Copy name", icon: icon(Copy), onClick: () => void copy(name, "Name") },
    SEPARATOR,
    ...serverItems(ws, name),
    SEPARATOR,
    { label: "New database…", icon: icon(Plus), onClick: () => flows.createDatabaseFlow(ws) },
    {
      label: "Drop database…",
      icon: icon(Trash2),
      danger: true,
      disabled: name === ws.info.database,
      onClick: () => flows.dropDatabase(ws, name),
    },
  ];
}

export function savedQueryMenu(ws: Workspace, s: SavedQuery, reload: () => void): MenuItem[] {
  const id = ws.connectionId;
  const thenReload = (done: Promise<boolean>) => done.then((ok) => ok && reload());
  return [
    { label: "Open", icon: icon(FileCode2), onClick: () => void openSavedQuery(id, s.name) },
    {
      label: "Rename…",
      icon: icon(Pencil),
      onClick: () => void thenReload(renameSavedQuery(id, s.name)),
    },
    {
      label: "Delete",
      icon: icon(Trash2),
      danger: true,
      onClick: () => void thenReload(deleteSavedQuery(id, s.name)),
    },
  ];
}

/** Tools for the tables of the current schema (Tables section, schema menu). */
export function tablesMenu(ws: Workspace): MenuItem[] {
  return [
    { label: "Open ER diagram", icon: icon(Network), onClick: () => tools.openDiagram(ws) },
    {
      label: "Import into new table…",
      icon: icon(FileUp),
      onClick: () => tools.openImport(ws, null),
    },
    { label: "Export tables…", icon: icon(Download), onClick: () => tools.openExportTables(ws) },
    { label: "Run SQL file…", icon: icon(FileCode2), onClick: () => tools.openSqlFile(ws) },
  ];
}

export function schemaMenu(ws: Workspace): MenuItem[] {
  return [
    ...tablesMenu(ws),
    SEPARATOR,
    { label: "New schema…", icon: icon(FolderPlus), onClick: () => flows.createSchema(ws) },
    {
      label: `Drop schema “${ws.schema}”…`,
      icon: icon(Trash2),
      danger: true,
      disabled: ws.schema === "public",
      onClick: () => flows.dropSchema(ws, ws.schema),
    },
  ];
}
