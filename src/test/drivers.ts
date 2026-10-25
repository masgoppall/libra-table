// Driver catalog and capabilities as the Rust side reports them, for tests.

import { type Capabilities, type DriverEntry, type OperatorInfo } from "@/types";

export function pgCapabilities(operators: OperatorInfo[] = []): Capabilities {
  return {
    operators,
    kind: "sql",
    sqlDialect: "postgres",
    queryLanguage: "sql",
    ready: true,
    hasSchemas: true,
    switchDatabase: true,
    query: true,
    editRows: true,
    structure: "edit",
    createTable: true,
    createDatabase: true,
    schemaObjects: true,
    processes: true,
    users: true,
    backup: true,
    import: true,
    diagram: true,
    explain: true,
    metrics: true,
    quickLook: true,
  };
}

const server = (id: DriverEntry["id"], label: string, port: number | null): DriverEntry => ({
  id,
  label,
  group: "relational",
  kind: "sql",
  sqlDialect: id,
  queryLanguage: "sql",
  defaultPort: port,
  fields: [
    { key: "host", label: "Host", kind: "text", placeholder: "127.0.0.1", required: false },
    { key: "port", label: "Port", kind: "number", placeholder: "", required: false },
    { key: "user", label: "User", kind: "text", placeholder: "", required: false },
    { key: "database", label: "Database", kind: "text", placeholder: "", required: false },
  ].map((f) => ({ ...f, kind: f.kind as "text", options: [], help: "" })),
  secret: { label: "Password", multiline: false, optional: true },
  ssh: true,
  tls: true,
  note: "",
  ready: true,
});

export const DRIVERS: DriverEntry[] = [
  server("postgres", "PostgreSQL", 5432),
  server("mysql", "MySQL", 3306),
  { ...server("sqlite", "SQLite", null), fields: [], secret: null, ssh: false, tls: false },
  {
    ...server("snowflake", "Snowflake", null),
    group: "cloud",
    fields: [
      {
        key: "extra.account",
        label: "Account",
        kind: "text",
        placeholder: "xy12345.eu-central-1",
        required: true,
        options: [],
        help: "",
      },
    ],
    ssh: false,
    tls: false,
  },
  { ...server("oracle", "Oracle", 1521), ready: false },
];
