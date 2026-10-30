// Saved connections, groups, secrets and sessions. Secrets only travel UI → Rust.

import {
  type ConnectInfo,
  type Connection,
  type ConnectionDraft,
  type ConnectionsFileSummary,
  type ConnectionView,
  type Group,
  type ImportedKeyInfo,
  type ImportReport,
  type NewTab,
  type PromptedSecrets,
  type SecretsInput,
  type TestResult,
} from "@/types";
import { call } from "./client";

export const connectionsApi = {
  listConnections: () =>
    call<{ connections: ConnectionView[]; groups: Group[] }>("list_connections"),
  saveConnection: (connection: Connection, secrets: SecretsInput) =>
    call<ConnectionView>("save_connection", { input: { connection, secrets } }),
  deleteConnection: (id: string) => call<void>("delete_connection", { id }),
  duplicateConnection: (id: string, copySecrets: boolean) =>
    call<ConnectionView>("duplicate_connection", { id, copySecrets }),
  saveGroup: (group: Group) => call<Group>("save_group", { group }),
  deleteGroup: (id: string) => call<void>("delete_group", { id }),
  importSshKey: (path: string) => call<ImportedKeyInfo>("import_ssh_key", { path }),
  testConnection: (connection: Connection, secrets: SecretsInput, prompted?: PromptedSecrets) =>
    call<TestResult>("test_connection", {
      input: { connection, secrets },
      prompted: prompted ?? null,
    }),
  connect: (id: string, prompted?: PromptedSecrets) =>
    call<ConnectInfo>("connect", { id, prompted: prompted ?? null }),
  /** This window lets go of the connection (closed once no window shows it); `force`
   * closes it anyway (Reconnect). */
  disconnect: (id: string, force = false) => call<void>("disconnect", { id, force }),
  /** Opens a connection in a window of its own, optionally with a tab moved there. */
  openWindow: (id: string, tab: NewTab | null = null) =>
    call<string>("open_window", { id, tab: tab ? JSON.stringify(tab) : null }),
  openSessions: () => call<string[]>("open_sessions"),
  trustHostKey: (host: string, port: number) => call<void>("trust_host_key", { host, port }),
  connectionUrl: (id: string) => call<string>("connection_url", { id }),
  parseConnectionUrl: (url: string) => call<ConnectionDraft>("parse_connection_url", { url }),
  exportConnections: (ids: string[], path: string, passphrase: string | null) =>
    call<number>("export_connections", { ids, path, passphrase }),
  inspectConnectionsFile: (path: string) =>
    call<ConnectionsFileSummary>("inspect_connections_file", { path }),
  importConnections: (path: string, passphrase: string | null) =>
    call<ImportReport>("import_connections", { path, passphrase }),
  createSqliteFile: (path: string) => call<void>("create_sqlite_file", { path }),
  closeDatabase: (id: string, database: string) => call<void>("close_database", { id, database }),
};
