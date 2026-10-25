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
import { openServerDialog, serverActionAvailable } from "../server/serverActions";
import { newQueryTab, tableTab } from "../workspace/tabs";

const icon = (Icon: typeof Copy) => <Icon size={14} />;

/** `items` when the driver offers them, else nothing. */
const when = (offered: boolean, ...items: MenuItem[]): MenuItem[] => (offered ? items : []);

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
  const select = selectAllSql(ws.info.dialect, ws.schema, o.name);
  const where = { schema: ws.schema };
  const dropView: MenuItem = {
    label: "Drop view…",
    icon: icon(Trash2),
    danger: true,
    onClick: () => flows.dropTable(ws, ws.schema, o.name, "view"),
  };
  const caps = ws.caps;
  const sql = caps.query && caps.sqlDialect !== null;
  const editable = caps.structure === "edit";
  return [
    {
      label: "Open data",
      icon: icon(Table2),
      onClick: () => addTab(id, tableTab("data", o.name, where), { reuse: true }),
    },
    ...when(caps.structure !== "none", {
      label: isTable && editable ? "Edit structure" : "Show structure",
      icon: icon(Columns3),
      onClick: () => addTab(id, tableTab("structure", o.name, where), { reuse: true }),
    }),
    ...when(sql, {
      label: "New query with SELECT",
      icon: icon(SquareTerminal),
      onClick: () => addTab(id, newQueryTab(`${select}\n`, { title: `Query · ${o.name}` })),
    }),
    SEPARATOR,
    { label: "Copy name", icon: icon(Copy), onClick: () => void copy(o.name, "Name") },
    ...when(sql, { label: "Copy SELECT … LIMIT 100", onClick: () => void copy(select, "SELECT") }),
    { label: "Export…", icon: icon(Download), onClick: () => onExport(ws.schema, o.name) },
    ...when(
      isTable,
      ...when(caps.import, {
        label: "Import…",
        icon: icon(FileUp),
        onClick: () => tools.openImport(ws, o.name),
      }),
      { label: "Export tables…", onClick: () => tools.openExportTables(ws, [o.name]) },
    ),
    ...extra,
    ...when(editable, SEPARATOR, ...(isTable ? tableItems(ws, o.name) : [dropView])),
  ];
}

/** Backup, restore and the command line for one database. */
function serverItems(ws: Workspace, name: string): MenuItem[] {
  const open = (kind: "backup" | "restore" | "command-line") => () =>
    openServerDialog(ws, kind, name);
  return [
    ...when(serverActionAvailable(ws, "backup"), {
      label: "Backup…",
      icon: icon(DatabaseBackup),
      onClick: open("backup"),
    }),
    ...when(serverActionAvailable(ws, "restore"), {
      label: "Restore…",
      icon: icon(ArchiveRestore),
      onClick: open("restore"),
    }),
    ...when(serverActionAvailable(ws, "command-line"), {
      label: "Open in command line",
      icon: icon(Terminal),
      onClick: open("command-line"),
    }),
  ];
}

export function databaseMenu(ws: Workspace, name: string): MenuItem[] {
  const id = ws.connectionId;
  const { addTab, switchDatabase } = store.get();
  const query = newQueryTab("", {
    title: `Query · ${name}`,
    database: name,
    schema: defaultSchema(ws.info.dialect, name),
  });
  const caps = ws.caps;
  const server = serverItems(ws, name);
  return [
    { label: "Open", icon: icon(Database), onClick: () => switchDatabase(id, name) },
    ...when(caps.query, {
      label: "New query in this database",
      icon: icon(SquareTerminal),
      onClick: () => addTab(id, query),
    }),
    ...when(caps.import, {
      label: "Run SQL file…",
      icon: icon(FileCode2),
      onClick: () => tools.openSqlFile(ws, name),
    }),
    { label: "Copy name", icon: icon(Copy), onClick: () => void copy(name, "Name") },
    ...when(server.length > 0, SEPARATOR, ...server),
    ...when(
      caps.createDatabase,
      SEPARATOR,
      { label: "New database…", icon: icon(Plus), onClick: () => flows.createDatabaseFlow(ws) },
      {
        label: "Drop database…",
        icon: icon(Trash2),
        danger: true,
        disabled: name === ws.info.database,
        onClick: () => flows.dropDatabase(ws, name),
      },
    ),
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
  const caps = ws.caps;
  return [
    ...when(caps.diagram, {
      label: "Open ER diagram",
      icon: icon(Network),
      onClick: () => tools.openDiagram(ws),
    }),
    ...when(caps.import && caps.createTable, {
      label: "Import into new table…",
      icon: icon(FileUp),
      onClick: () => tools.openImport(ws, null),
    }),
    { label: "Export tables…", icon: icon(Download), onClick: () => tools.openExportTables(ws) },
    ...when(caps.import, {
      label: "Run SQL file…",
      icon: icon(FileCode2),
      onClick: () => tools.openSqlFile(ws),
    }),
  ];
}

export function schemaMenu(ws: Workspace): MenuItem[] {
  return [
    ...tablesMenu(ws),
    ...when(
      ws.caps.createDatabase,
      SEPARATOR,
      { label: "New schema…", icon: icon(FolderPlus), onClick: () => flows.createSchema(ws) },
      {
        label: `Drop schema “${ws.schema}”…`,
        icon: icon(Trash2),
        danger: true,
        disabled: ws.schema === "public",
        onClick: () => flows.dropSchema(ws, ws.schema),
      },
    ),
  ];
}
