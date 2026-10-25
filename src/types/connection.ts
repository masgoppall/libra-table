// Saved connections and what connecting returns. Mirrors src-tauri models (serde camelCase).

export type Driver = "postgres" | "mysql" | "sqlite";
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
}

export interface ConnectionDraft {
  connection: Connection;
  dbPassword: string | null;
  sshPassword: string | null;
}
