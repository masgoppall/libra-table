// Server tools: process list, users & privileges, backup / restore, command line.
// Mirrors src-tauri/src/server (serde camelCase; enum tags snake_case).

import { type AppError } from "./errors";

export type ProcessAction = "cancel" | "kill";

export interface ProcessInfo {
  /** Backend pid (Postgres) or connection id (MySQL). */
  id: number;
  user: string;
  database: string;
  client: string;
  application: string;
  state: string;
  durationMs: number | null;
  query: string;
  /** LibraTable's own connection. */
  own: boolean;
}

export interface PgRole {
  name: string;
  login: boolean;
  superuser: boolean;
  createDb: boolean;
  createRole: boolean;
  replication: boolean;
  /** -1 means no limit. */
  connLimit: number;
  validUntil: string | null;
  memberOf: string[];
}

export interface MysqlAccount {
  user: string;
  host: string;
  /** Global (`*.*`) privileges. */
  privileges: string[];
}

export type TargetKind = "global" | "database" | "schema" | "all_tables" | "table";

export interface PrivilegeOption {
  target: TargetKind;
  privileges: string[];
}

export interface UserList {
  roles: PgRole[];
  accounts: MysqlAccount[];
  privilegeOptions: PrivilegeOption[];
}

export interface RoleSpec {
  name: string;
  login: boolean;
  superuser: boolean;
  createDb: boolean;
  createRole: boolean;
  replication: boolean;
  /** null means no limit. */
  connLimit: number | null;
  validUntil: string | null;
  memberOf: string[];
}

/** A role (Postgres) or `user@host` (MySQL). */
export interface Grantee {
  name: string;
  host?: string | null;
}

export type GrantTarget =
  | { on: "global" }
  | { on: "database"; name: string }
  | { on: "schema"; name: string }
  | { on: "all_tables"; schema: string }
  | { on: "table"; schema: string; name: string };

/** Passwords go to Rust once, for this change; previews show them masked. */
export type UserChange =
  | { kind: "create_role"; role: RoleSpec; password?: string | null }
  | { kind: "alter_role"; name: string; role: RoleSpec; password?: string | null }
  | { kind: "drop_role"; name: string }
  | { kind: "create_user"; user: string; host: string; password?: string | null }
  | { kind: "drop_user"; user: string; host: string }
  | { kind: "set_password"; grantee: Grantee; password: string }
  | {
      kind: "grant";
      grantee: Grantee;
      privileges: string[];
      target: GrantTarget;
      withGrantOption?: boolean;
    }
  | { kind: "revoke"; grantee: Grantee; privileges: string[]; target: GrantTarget };

export type DumpFormat = "plain" | "custom";
export type DumpContent = "all" | "schema_only" | "data_only";

export interface TableRef {
  schema: string;
  name: string;
}

export interface BackupRequest {
  path: string;
  format: DumpFormat;
  content: DumpContent;
  schemas: string[];
  tables: TableRef[];
  noOwner: boolean;
  noPrivileges: boolean;
  clean: boolean;
  createDatabase: boolean;
  singleTransaction: boolean;
  routines: boolean;
  triggers: boolean;
  events: boolean;
}

export interface RestoreRequest {
  path: string;
  stopOnError: boolean;
  singleTransaction: boolean;
  clean: boolean;
  noOwner: boolean;
}

export interface ToolCandidate {
  path: string;
  version: string;
  major: number;
  mariadb: boolean;
}

export interface ToolChoice {
  tool: string;
  path: string | null;
  version: string | null;
  major: number | null;
  mariadb: boolean;
  /** The user chose this path. */
  custom: boolean;
  warning: string | null;
  candidates: ToolCandidate[];
}

/** The tool and the command it will run (no password in it). SQLite has no tool. */
export interface ToolPlan {
  tool: ToolChoice | null;
  command: string;
}

export interface ToolProgress {
  jobId: string;
  /** A line the tool printed, when the event carries one. */
  line: string | null;
  bytes: number;
  total: number | null;
  done: boolean;
  error: AppError | null;
  path: string;
}

export interface CommandLine {
  command: string;
  tool: ToolChoice;
  /** Uses the SSH tunnel's local port: works while LibraTable is connected. */
  throughTunnel: boolean;
}
