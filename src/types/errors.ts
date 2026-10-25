// Errors from Rust (src-tauri/src/error.rs). `data` depends on the kind.

export type ErrorKind =
  | "ssh_unreachable"
  | "ssh_auth_failed"
  | "ssh_host_key_unknown"
  | "ssh_host_key_changed"
  | "ssh_passphrase_required"
  | "ssh_passphrase_wrong"
  | "ssh_key_invalid"
  | "tunnel_failed"
  | "db_connect"
  | "db_auth"
  | "db_not_found"
  | "tls"
  | "query"
  | "not_connected"
  | "confirmation_required"
  | "read_only"
  | "password_required"
  | "validation"
  | "not_found"
  | "io"
  | "keychain"
  | "cancelled"
  | "internal";

export type ErrorLayer = "ssh" | "database" | "app";

export interface HostKeyData {
  host: string;
  port: number;
  algorithm: string;
  fingerprint: string;
}

export interface PasswordRequiredData {
  which: "ssh" | "database";
  connectionName?: string;
}

export interface PassphraseData {
  keyName?: string;
}

export interface ConfirmationData {
  statements: { sql: string; understood: boolean }[];
  /** "safe": type the connection name to confirm. */
  level?: "alert" | "safe";
}

export interface QueryErrorData {
  /** SQLSTATE or the driver's error code. */
  code?: string | null;
}

interface BaseError<K extends ErrorKind, D = undefined> {
  kind: K;
  layer: ErrorLayer;
  message: string;
  hint?: string;
  data?: D;
}

export type HostKeyError = BaseError<"ssh_host_key_unknown" | "ssh_host_key_changed", HostKeyData>;
export type PasswordRequiredError = BaseError<"password_required", PasswordRequiredData>;
export type PassphraseError = BaseError<
  "ssh_passphrase_required" | "ssh_passphrase_wrong",
  PassphraseData
>;
export type ConfirmationError = BaseError<"confirmation_required", ConfirmationData>;
export type QueryError = BaseError<"query", QueryErrorData>;

type KindsWithData =
  | HostKeyError["kind"]
  | PasswordRequiredError["kind"]
  | PassphraseError["kind"]
  | ConfirmationError["kind"]
  | QueryError["kind"];

export type AppError =
  | HostKeyError
  | PasswordRequiredError
  | PassphraseError
  | ConfirmationError
  | QueryError
  | BaseError<Exclude<ErrorKind, KindsWithData>>;
