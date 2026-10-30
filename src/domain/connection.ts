// Connection defaults and presentation: drivers, tags and their colors.

import {
  type Connection,
  type ConnectionView,
  type Dialect,
  type Driver,
  type DriverGroup,
  type SecretChange,
  type SecretsInput,
  type Tag,
} from "@/types";

export const DRIVER_LABEL: Record<Driver, string> = {
  postgres: "PostgreSQL",
  mysql: "MySQL",
  sqlite: "SQLite",
  mariadb: "MariaDB",
  cockroach: "CockroachDB",
  redshift: "Amazon Redshift",
  greenplum: "Greenplum",
  vertica: "Vertica",
  libsql: "LibSQL / Turso",
  cloudflare_d1: "Cloudflare D1",
  mssql: "Microsoft SQL Server",
  oracle: "Oracle",
  clickhouse: "ClickHouse",
  duckdb: "DuckDB",
  cassandra: "Cassandra",
  snowflake: "Snowflake",
  bigquery: "BigQuery",
  redis: "Redis",
  mongodb: "MongoDB",
  dynamodb: "DynamoDB",
  elasticsearch: "Elasticsearch",
};

/** Drivers on the built-in Postgres, MySQL and SQLite code (the full feature set). */
const BUILT_IN: Partial<Record<Driver, Dialect>> = {
  postgres: "postgres",
  cockroach: "postgres",
  redshift: "postgres",
  greenplum: "postgres",
  vertica: "postgres",
  mysql: "mysql",
  mariadb: "mysql",
  sqlite: "sqlite",
};

/** The built-in dialect a driver runs on, if it is one of them (mirrors Driver::sqlx_dialect). */
export const builtInDialect = (driver: Driver): Dialect | undefined => BUILT_IN[driver];

/** Default ports of the built-in server drivers (the form's placeholder). */
export const DEFAULT_PORT: Partial<Record<Driver, number>> = {
  postgres: 5432,
  greenplum: 5432,
  cockroach: 26257,
  redshift: 5439,
  vertica: 5433,
  mysql: 3306,
  mariadb: 3306,
};

/** Drivers that open a local file. */
export const isFileDriver = (driver: Driver) => driver === "sqlite" || driver === "duckdb";

/** Picker sections, in order. */
export const DRIVER_GROUPS: { id: DriverGroup; label: string }[] = [
  { id: "relational", label: "Relational" },
  { id: "compatible", label: "Compatible" },
  { id: "analytics", label: "Analytics" },
  { id: "cloud", label: "Cloud" },
  { id: "nosql", label: "NoSQL" },
];

export const TAGS: Tag[] = ["local", "testing", "development", "staging", "production"];

/** Most sensitive first, for "Sort by tag". */
export const TAG_ORDER: Tag[] = ["production", "staging", "development", "testing", "local"];

export const TAG_COLORS: Record<Tag, string> = {
  local: "#30a46c",
  testing: "#3e63dd",
  development: "#8e4ec6",
  staging: "#f76b15",
  production: "#e5484d",
};

export function connectionColor(c: Pick<Connection, "color" | "tag">): string {
  return c.color || TAG_COLORS[c.tag];
}

export function isProduction(c: Pick<Connection, "tag"> | undefined): boolean {
  return c?.tag === "production";
}

export function newConnection(driver: Driver): Connection {
  return {
    id: "",
    name: "",
    groupId: null,
    driver,
    tag: "local",
    color: "",
    db: {
      host: "",
      port: null,
      user: "",
      database: "",
      useSocket: false,
      socket: "",
      filePath: "",
      passwordMode: "keychain",
      extra: {},
    },
    ssh: {
      enabled: false,
      host: "",
      port: null,
      user: "",
      passwordMode: "keychain",
      useKey: false,
      keyName: "",
    },
    tls: { mode: "prefer", caPath: "", certPath: "", keyPath: "" },
    options: {
      showSystemSchemas: false,
      pageSize: 300,
      queryRowLimit: 1000,
      safeMode: "auto",
      readOnly: false,
      startupQuery: "",
      keepAliveSecs: 0,
    },
    createdAt: 0,
    updatedAt: 0,
  };
}

export const KEEP: SecretChange = { action: "keep" };

export function emptySecrets(): SecretsInput {
  return {
    dbPassword: KEEP,
    sshPassword: KEEP,
    sshKeyToken: null,
    clearSshKey: false,
    sshKeyPassphrase: KEEP,
  };
}

/** Text the connection search matches against. */
/** The second line of a connection in the list, as TablePlus shows it. */
export function connectionSubtitle(c: ConnectionView): string {
  if (isFileDriver(c.driver)) return c.db.filePath;
  if (c.ssh.enabled) return `SSH : ${c.db.database || c.ssh.host}`;
  if (!builtInDialect(c.driver) || c.db.useSocket) return c.summary;
  return c.db.host.trim() || "127.0.0.1";
}

export function searchText(c: ConnectionView): string {
  return `${c.name} ${c.summary} ${c.driver} ${c.tag} ${(c.labels ?? []).join(" ")}`;
}
