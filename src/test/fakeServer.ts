// Fake answers for the server tools commands (process list, users, backup / restore,
// command line), used by FakeBackend.

import { type ProcessInfo, type ToolChoice, type UserList } from "@/types";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Args = Record<string, any>;

export const PROCESSES: ProcessInfo[] = [
  {
    id: 101,
    user: "app",
    database: "app",
    client: "10.0.0.9:51000",
    application: "worker",
    state: "active",
    durationMs: 65_000,
    query: "SELECT pg_sleep(600)",
    own: false,
  },
  {
    id: 102,
    user: "postgres",
    database: "app",
    client: "127.0.0.1:50000",
    application: "LibraTable",
    state: "idle",
    durationMs: 2_000,
    query: "SELECT 1",
    own: true,
  },
];

export const USERS: UserList = {
  roles: [
    {
      name: "reader",
      login: true,
      superuser: false,
      createDb: false,
      createRole: false,
      replication: false,
      connLimit: -1,
      validUntil: null,
      memberOf: [],
    },
  ],
  accounts: [],
  privilegeOptions: [
    { target: "database", privileges: ["CONNECT", "CREATE", "ALL PRIVILEGES"] },
    { target: "table", privileges: ["SELECT", "INSERT", "ALL PRIVILEGES"] },
  ],
};

export const PG_DUMP: ToolChoice = {
  tool: "pg_dump",
  path: "/opt/homebrew/bin/pg_dump",
  version: "pg_dump (PostgreSQL) 16.4",
  major: 16,
  mariadb: false,
  custom: false,
  warning: null,
  candidates: [
    {
      path: "/opt/homebrew/bin/pg_dump",
      version: "pg_dump (PostgreSQL) 16.4",
      major: 16,
      mariadb: false,
    },
  ],
};

const tool = (name: string): ToolChoice => ({ ...PG_DUMP, tool: name, path: `/usr/bin/${name}` });

/** A preview like Rust's: the password masked. */
function previewUser(change: Args): string[] {
  if (change.kind === "create_role") {
    const pw = change.password ? " PASSWORD '********'" : "";
    return [`CREATE ROLE "${change.role.name}" WITH LOGIN${pw}`];
  }
  if (change.kind === "grant") {
    return [`GRANT ${change.privileges.join(", ")} ON … TO "${change.grantee.name}"`];
  }
  return [`-- ${change.kind}`];
}

export const serverHandlers: Record<string, (args: Args) => unknown> = {
  list_processes: () => PROCESSES,
  preview_process_action: (a) => [
    a.action === "cancel"
      ? `SELECT pg_cancel_backend(${a.processId})`
      : `SELECT pg_terminate_backend(${a.processId})`,
  ],
  run_process_action: () => null,
  list_users: () => USERS,
  user_grants: () => ["CONNECT ON DATABASE app"],
  preview_user_change: (a) => previewUser(a.change),
  apply_user_change: (a) => previewUser(a.change),
  plan_backup: (a) => ({
    tool: PG_DUMP,
    command: `/opt/homebrew/bin/pg_dump -h 127.0.0.1 --file=${a.request.path}`,
  }),
  plan_restore: (a) => ({
    tool: tool("psql"),
    command: `/usr/bin/psql --dbname=app < ${a.request.path}`,
  }),
  start_backup: () => null,
  start_restore: () => null,
  set_tool_path: () => null,
  cancel_tool_job: () => true,
  command_line: () => ({
    command: "/usr/bin/psql -h 127.0.0.1 -p 5432 -U postgres -d app",
    tool: tool("psql"),
    throughTunnel: false,
  }),
  open_in_terminal: () => null,
  // File pickers answer with a fixed path.
  "plugin:dialog|save": () => "/tmp/app-backup.sql",
  "plugin:dialog|open": () => "/tmp/app-backup.sql",
};
