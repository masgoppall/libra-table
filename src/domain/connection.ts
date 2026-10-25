// Connection defaults and presentation: drivers, tags and their colors.

import {
  type Connection,
  type ConnectionView,
  type Driver,
  type SecretChange,
  type SecretsInput,
  type Tag,
} from "@/types";

export interface DriverInfo {
  id: Driver;
  label: string;
  description: string;
  defaultPort: number | null;
}

export const DRIVERS: DriverInfo[] = [
  {
    id: "postgres",
    label: "PostgreSQL",
    description: "PostgreSQL 10 and later",
    defaultPort: 5432,
  },
  {
    id: "mysql",
    label: "MySQL / MariaDB",
    description: "MySQL 5.7+ and MariaDB 10.3+",
    defaultPort: 3306,
  },
  { id: "sqlite", label: "SQLite", description: "A database file on this Mac", defaultPort: null },
];

export const DRIVER_LABEL: Record<Driver, string> = {
  postgres: "PostgreSQL",
  mysql: "MySQL",
  sqlite: "SQLite",
};

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
export function searchText(c: ConnectionView): string {
  return `${c.name} ${c.summary} ${c.driver} ${c.tag}`;
}
