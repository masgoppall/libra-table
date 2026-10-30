// Saved connections and what connecting returns. Mirrors src-tauri models (serde camelCase).

/** The database product of a connection (src-tauri/src/models.rs `Driver`). */
export type Driver =
  | "postgres"
  | "mysql"
  | "sqlite"
  | "mariadb"
  | "cockroach"
  | "redshift"
  | "greenplum"
  | "vertica"
  | "libsql"
  | "cloudflare_d1"
  | "mssql"
  | "oracle"
  | "clickhouse"
  | "duckdb"
  | "cassandra"
  | "snowflake"
  | "bigquery"
  | "redis"
  | "mongodb"
  | "dynamodb"
  | "elasticsearch";

/** SQL rules for quoting and splitting: the built-in dialect closest to the driver. */
export type Dialect = "postgres" | "mysql" | "sqlite";
export type Tag = "local" | "testing" | "development" | "staging" | "production";
export type PasswordMode = "keychain" | "ask" | "none";
export type TlsMode = "disable" | "allow" | "prefer" | "require" | "verify_ca" | "verify_full";

export interface DbSettings {
  host: string;
  port: number | null;
  user: string;
  database: string;
  useSocket: boolean;
  socket: string;
  filePath: string;
  passwordMode: PasswordMode;
  /** Driver-specific settings that are not secrets (account, region, URL…). */
  extra: Record<string, string>;
}

export interface SshSettings {
  enabled: boolean;
  host: string;
  port: number | null;
  user: string;
  passwordMode: PasswordMode;
  useKey: boolean;
  keyName: string;
}

export interface TlsSettings {
  mode: TlsMode;
  caPath: string;
  certPath: string;
  keyPath: string;
}

/** When statements need a confirmation; "auto" confirms writes on production only. */
export type SafeMode =
  "auto" | "silent" | "alert_writes" | "alert_all" | "safe_writes" | "safe_all";

export interface ConnectionOptions {
  showSystemSchemas: boolean;
  pageSize: number;
  queryRowLimit: number;
  safeMode: SafeMode;
  /** Refuse writes (also enforced on the server session). */
  readOnly: boolean;
  /** SQL run on every new connection (SET search_path, time zone…). */
  startupQuery: string;
  /** Ping interval keeping idle connections and SSH tunnels open; 0 = off. */
  keepAliveSecs: number;
}

export interface Connection {
  id: string;
  name: string;
  groupId: string | null;
  driver: Driver;
  tag: Tag;
  color: string;
  /** Custom tags (Settings › Tags) on top of the environment tag. */
  labels?: string[];
  db: DbSettings;
  ssh: SshSettings;
  tls: TlsSettings;
  options: ConnectionOptions;
  createdAt: number;
  updatedAt: number;
}

/** A connection as listed: secrets are never sent, only whether they exist. */
export interface ConnectionView extends Connection {
  summary: string;
  hasDbPassword: boolean;
  hasSshPassword: boolean;
  hasSshKey: boolean;
  hasSshKeyPassphrase: boolean;
}

export interface Group {
  id: string;
  name: string;
  parentId: string | null;
  collapsed: boolean;
  /** A custom icon (a small `data:image/…` URL); the plug when null. */
  icon?: string | null;
}

export type SecretChange =
  { action: "keep" } | { action: "set"; value: string } | { action: "clear" };

export interface SecretsInput {
  dbPassword: SecretChange;
  sshPassword: SecretChange;
  sshKeyToken: string | null;
  clearSshKey: boolean;
  sshKeyPassphrase: SecretChange;
}

/** Secrets typed into a prompt while connecting; used once, saved only if asked. */
export interface PromptedSecrets {
  dbPassword?: string;
  sshPassword?: string;
  sshKeyPassphrase?: string;
  savePassphrase?: boolean;
}

export interface ConnectInfo {
  connectionId: string;
  driver: Driver;
  dialect: Dialect;
  database: string;
  serverVersion: string;
  throughTunnel: boolean;
}

export interface TestResult {
  serverVersion: string;
  durationMs: number;
  warnings: { code: string; message: string }[];
}

export interface ImportedKeyInfo {
  token: string;
  name: string;
  encrypted: boolean;
}

/** What a connections file holds, before importing it. */
export interface ConnectionsFileSummary {
  names: string[];
  hasSecrets: boolean;
  source: "libratable" | "tableplus";
  /** Encrypted as a whole (TablePlus): the names are known only after the password. */
  locked: boolean;
}

export interface ImportReport {
  added: number;
  /** Left out, with why ("already in LibraTable", an unsupported driver…). */
  skipped: { name: string; reason: string }[];
}

export interface ConnectionDraft {
  connection: Connection;
  dbPassword: string | null;
  sshPassword: string | null;
}

export type DriverGroup = "relational" | "compatible" | "analytics" | "cloud" | "nosql";

/** How a workspace presents data: tables and SQL, documents, or keys. */
export type DataKind = "sql" | "document" | "key_value";

export type FieldKind = "text" | "number" | "file" | "select" | "bool";

/** A connection form field; `key` is host, port, user, database, filePath or extra.<name>. */
export interface DriverField {
  key: string;
  label: string;
  kind: FieldKind;
  placeholder: string;
  required: boolean;
  options: string[];
  help: string;
}

/** The secret kept in the keychain for the driver (password, token, key…). */
export interface DriverSecret {
  label: string;
  multiline: boolean;
  optional: boolean;
}

/** One entry of the driver picker (src-tauri/src/drivers.rs). */
export interface DriverEntry {
  id: Driver;
  label: string;
  group: DriverGroup;
  kind: DataKind;
  sqlDialect: string | null;
  queryLanguage: string;
  defaultPort: number | null;
  fields: DriverField[];
  secret: DriverSecret | null;
  ssh: boolean;
  tls: boolean;
  note: string;
  ready: boolean;
}
