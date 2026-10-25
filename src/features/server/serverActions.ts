// Process list, users, backup / restore and the command line: where each opens and
// which drivers have it (SQLite has no server: only backup and the command line).

import { type ActionId, LAYER, useActions } from "@/lib/actions";
import { toastInfo } from "@/services/errors";
import { type ServerDialog, store } from "@/store";
import { type NewTab, type Workspace } from "@/types";

export const SERVER_ACTIONS: ActionId[] = [
  "process-list",
  "users",
  "backup",
  "restore",
  "command-line",
];

const SERVER_ONLY: ActionId[] = ["process-list", "users", "restore"];

/** False for the server-only tools on SQLite (hidden from the palette). */
export function serverActionAvailable(ws: Workspace, id: ActionId): boolean {
  return ws.info.driver !== "sqlite" || !SERVER_ONLY.includes(id);
}

/** The process list is server-wide: one tab, whatever database the sidebar shows. */
export const processesTab = (ws: Workspace): NewTab => ({
  kind: "processes",
  title: "Processes",
  database: ws.info.database,
});

export const usersTab = (database: string): NewTab => ({ kind: "users", title: "Users", database });

export function openServerDialog(
  ws: Workspace,
  kind: ServerDialog["kind"],
  database: string = ws.database,
) {
  store.get().setServerDialog({ kind, connectionId: ws.connectionId, database });
}

const SQLITE_NOTES: Partial<Record<ActionId, [string, string]>> = {
  "process-list": ["SQLite has no server, so there is no process list.", ""],
  users: ["SQLite has no users or privileges.", ""],
  restore: [
    "Restore isn't offered for SQLite.",
    "A backup is a database file: open it as a connection, or copy it over the original " +
      "while it's disconnected.",
  ],
};

export function useServerActions(ws: Workspace, visible: boolean) {
  const id = ws.connectionId;
  const sqliteNote = (action: ActionId) => {
    const [message, detail] = SQLITE_NOTES[action] ?? ["", ""];
    toastInfo(message, detail || undefined);
  };
  const guarded = (action: ActionId, run: () => void) => () =>
    serverActionAvailable(ws, action) ? run() : sqliteNote(action);
  const { addTab } = store.get();
  useActions(LAYER.workspace, visible, {
    "process-list": guarded("process-list", () => addTab(id, processesTab(ws), { reuse: true })),
    users: guarded("users", () => addTab(id, usersTab(ws.database), { reuse: true })),
    backup: () => openServerDialog(ws, "backup"),
    restore: guarded("restore", () => openServerDialog(ws, "restore")),
    "command-line": () => openServerDialog(ws, "command-line"),
  });
}
