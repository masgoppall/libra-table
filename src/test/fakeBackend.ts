// An in-memory stand-in for the Rust commands, recording every call.

import { mockIPC } from "@tauri-apps/api/mocks";
import { newConnection } from "@/domain/connection";
import {
  type ColumnInfo,
  type ConnectInfo,
  type ConnectionView,
  type Group,
  type OperatorInfo,
  type Settings,
} from "@/types";
import {
  objectHandlers,
  type ObjectsState,
  okResult,
  SCHEMA_OBJECTS,
  splitStatements,
} from "./fakeObjects";
import { serverHandlers } from "./fakeServer";
import { DRIVERS, pgCapabilities } from "./drivers";
import { FakeTools } from "./fakeTools";

/** Arguments as the command received them (shape depends on the command). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Args = Record<string, any>;

export interface Call {
  cmd: string;
  args: Args;
}

export function connectionView(
  overrides: Partial<ConnectionView> & { id: string; name: string },
): ConnectionView {
  return {
    ...newConnection("postgres"),
    summary: "127.0.0.1:5432/app",
    hasDbPassword: false,
    hasSshPassword: false,
    hasSshKey: false,
    hasSshKeyPassphrase: false,
    ...overrides,
  };
}

export const OPERATORS: OperatorInfo[] = [
  { id: "eq", label: "=", valueKind: "single", placeholder: "value" },
  { id: "contains", label: "Contains", valueKind: "single", placeholder: "text" },
  { id: "is_null", label: "IS NULL", valueKind: "none", placeholder: "" },
];

const column = (name: string, dataType: string, extra: Partial<ColumnInfo> = {}): ColumnInfo => ({
  name,
  dataType,
  nullable: false,
  default: null,
  pkPosition: null,
  autoIncrement: false,
  comment: null,
  ...extra,
});

export const COLUMNS: ColumnInfo[] = [
  column("id", "integer", { pkPosition: 1, autoIncrement: true }),
  column("name", "text"),
  column("note", "text", { nullable: true }),
];

const ORDER_COLUMNS: ColumnInfo[] = [
  column("id", "integer", { pkPosition: 1, autoIncrement: true }),
  column("person_id", "integer"),
  column("status", "order_status"),
  column("meta", "jsonb", { nullable: true }),
];

const ORDERS_PAGE = {
  columns: ORDER_COLUMNS,
  rows: [[10, 2, "new", '{"total":5,"items":["a"]}']],
  primaryKey: ["id"],
  page: 0,
  pageSize: 300,
  hasMore: false,
  sql: 'SELECT * FROM "public"."orders"',
  elapsedMs: 2,
};

const ORDERS_STRUCTURE = {
  kind: "table",
  columns: ORDER_COLUMNS,
  indexes: [],
  foreignKeys: [
    {
      name: "orders_person_fk",
      columns: ["person_id"],
      refTable: "people",
      refColumns: ["id"],
      onUpdate: "NO ACTION",
      onDelete: "CASCADE",
      definition: "FOREIGN KEY (person_id) REFERENCES people(id)",
    },
  ],
  ddl: 'CREATE TABLE "public"."orders" (...)',
};

const SETTINGS: Settings = {
  historyRetentionDays: 365,
  defaultFilterOperator: "eq",
  defaultFilterColumn: "first",
  theme: "system",
  editorFontSize: 13,
  uiFontSize: 13,
  lockAfterMinutes: 0,
  shortcuts: {},
  openInNewTab: true,
  alternatingRows: true,
  showRowNumbers: true,
  rowDensity: "normal",
  editorKeymap: "default",
  uppercaseKeywords: false,
  closeBrackets: true,
  customTags: [],
  variableFormat: "colon",
  variablesInStrings: false,
  colorTheme: "",
  customThemes: [],
  aiEnabled: false,
  aiModel: "claude-opus-5-5",
  backupSchedules: [],
};

const info = (id: string): ConnectInfo => ({
  connectionId: id,
  driver: "postgres",
  dialect: "postgres",
  database: "app",
  serverVersion: "16.4",
  throughTunnel: false,
});

export class FakeBackend {
  calls: Call[] = [];
  workspace: Args = {};
  databases = ["app", "analytics"];
  groups: Group[] = [];
  connections: ConnectionView[] = [
    connectionView({ id: "c-local", name: "Local Postgres" }),
    connectionView({
      id: "c-prod",
      name: "Prod Shop",
      tag: "production",
      summary: "10.0.0.5:5432/shop",
    }),
  ];
  passcode: string | null = null;
  /** What the save-file dialog answers. */
  savePath = "/tmp/app-backup.sql";
  console = [
    { at: 0, sql: "SELECT 1", rowsAffected: 0, rowsReturned: 1, elapsedMs: 0.4, slow: false },
    {
      at: 1,
      sql: "UPDATE t SET a = 1",
      rowsAffected: 3,
      rowsReturned: 0,
      elapsedMs: 1500,
      slow: true,
    },
  ];
  /** The system clipboard as the clipboard plugin sees it. */
  clipboard = "";
  settings: Settings = { ...SETTINGS };
  savedQueries: { name: string; modifiedAt: number; size: number }[] = [];
  /** Answers for the next `connect` calls (throw to fail one). */
  connectScript: Array<(args: Args) => unknown> = [];
  /** Imports, exports, diagram and metrics. */
  tools = new FakeTools();
  /** Functions, triggers…, recent/pinned items and SQL runs (see fakeObjects). */
  objects: ObjectsState = {
    schemaObjects: SCHEMA_OBJECTS,
    sidebar: { recent: [], pinned: [] },
    runQuery: (a) => splitStatements(a.sql).map((st) => okResult(st.text)),
  };

  install() {
    mockIPC((cmd, args) => this.handle(cmd, (args ?? {}) as Args), { shouldMockEvents: true });
  }

  callsOf(cmd: string): Call[] {
    return this.calls.filter((c) => c.cmd === cmd);
  }

  lastCall(cmd: string): Call | undefined {
    return this.callsOf(cmd).at(-1);
  }

  private handle(cmd: string, args: Args): unknown {
    this.calls.push({ cmd, args });
    const handler = this.handlers[cmd];
    return handler ? handler(args) : null;
  }

  private page = (args: Args) =>
    args.request.table === "orders"
      ? ORDERS_PAGE
      : {
          columns: COLUMNS,
          rows: [
            [1, "Alice", "likes 50%_off deals"],
            [2, "Bob", null],
          ],
          primaryKey: ["id"],
          page: 0,
          pageSize: 300,
          hasMore: false,
          sql: `SELECT "id", "name", "note" FROM "public"."${args.request.table}" LIMIT 300`,
          elapsedMs: 3,
        };

  private structure = (args: Args) =>
    args.table === "orders"
      ? ORDERS_STRUCTURE
      : {
          kind: "table",
          columns: COLUMNS,
          indexes: [
            {
              name: "people_pkey",
              columns: ["id"],
              unique: true,
              primary: true,
              definition: "CREATE UNIQUE INDEX people_pkey ON public.people USING btree (id)",
            },
          ],
          foreignKeys: [],
          ddl: 'CREATE TABLE "public"."people" (...)',
        };

  private handlers: Record<string, (args: Args) => unknown> = {
    ...serverHandlers,
    ...this.tools.handlers,
    list_connections: () => ({ connections: this.connections, groups: this.groups }),
    save_group: (a) => {
      const g: Group = { ...a.group, id: a.group.id || `g-${this.groups.length + 1}` };
      this.groups = [...this.groups.filter((x) => x.id !== g.id), g];
      return g;
    },
    console_entries: () => this.console,
    set_window_view: () => null,
    set_window_mode: () => null,
    app_info: () => ({ version: "0.4.0", dataDir: "/tmp/libratable" }),
    passcode_set: () => this.passcode !== null,
    verify_passcode: (a) => a.code === this.passcode,
    "plugin:dialog|save": () => this.savePath,
    "plugin:clipboard-manager|write_text": (a) => {
      this.clipboard = a.text;
      return null;
    },
    "plugin:clipboard-manager|read_text": () => this.clipboard,
    export_connections: (a) => (a.ids.length ? a.ids.length : this.connections.length),
    get_settings: () => this.settings,
    save_settings: (a) => (this.settings = a.settings),
    get_workspace: () => this.workspace,
    save_workspace: (a) => {
      this.workspace = a.workspace;
      return null;
    },
    driver_capabilities: () => pgCapabilities(OPERATORS),
    list_drivers: () => DRIVERS,
    connect: (a) => {
      const step = this.connectScript.shift();
      return step ? step(a) : info(a.id);
    },
    list_databases: () => this.databases,
    list_schemas: () => ["public"],
    list_objects: (a) =>
      a.database === "analytics"
        ? [{ name: "events", kind: "table" }]
        : [
            { name: "people", kind: "table" },
            { name: "orders", kind: "table" },
            { name: "active_people", kind: "view" },
          ],
    list_columns: () => COLUMNS,
    column_choices: (a) => (a.table === "orders" ? { status: ["new", "paid"] } : {}),
    fetch_cell: () => ({ kind: "text", text: '{"total":5,"items":["a"]}' }),
    list_saved_queries: () => this.savedQueries,
    ai_key_status: () => true,
    open_window: () => "win-1",
    ai_ask: (a) => ({
      sql: "SELECT name FROM people;",
      note: `Asked: ${a.request.question}`,
      model: a.request.model,
    }),
    completion_metadata: () => ({ schema: "public", schemas: ["public"], tables: {} }),
    fetch_page: this.page,
    count_rows: () => ({ total: 2, exact: true }),
    table_structure: this.structure,
    preview_changes: (a) =>
      a.changes.map(
        (c: Args) => `UPDATE "public"."people" SET ${Object.keys(c.values ?? {}).join(", ")} …`,
      ),
    commit_changes: (a) => {
      if (!a.confirmed) {
        throw {
          kind: "confirmation_required",
          layer: "app",
          message: "confirm",
          data: { statements: [] },
        };
      }
      return { affected: a.changes.length, statements: [] };
    },
    preview_schema_change: (a) => [`-- ${a.change.kind} ${a.change.name ?? ""}`.trim()],
    apply_schema_change: (a) => {
      if (a.change.kind === "create_database") this.databases = [...this.databases, a.change.name];
      return [`-- ${a.change.kind}`];
    },
    save_connection: (a) =>
      connectionView({ ...a.input.connection, id: a.input.connection.id || "c-new" }),
    ...objectHandlers(this.objects),
  };
}
