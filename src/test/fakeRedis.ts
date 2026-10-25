// An in-memory Redis for UI tests: a Redis connection, its capabilities, the key-view
// commands and the editor, layered over FakeBackend.

import { mockIPC } from "@tauri-apps/api/mocks";
import {
  type DatabaseInfo,
  type KeyData,
  type KeyEdit,
  type KeyRow,
} from "@/features/keyvalue/types";
import { type Capabilities, type ConnectInfo, type StatementResult } from "@/types";
import { pgCapabilities } from "./drivers";
import { type Args, connectionView, type FakeBackend } from "./fakeBackend";

type Value =
  | { type: "string"; value: string }
  | { type: "hash"; value: Record<string, string> }
  | { type: "list"; value: string[] }
  | { type: "set"; value: string[] }
  | { type: "zset"; value: [string, number][] };

interface Entry {
  data: Value;
  ttl: number;
}

export const REDIS_CAPS: Capabilities = {
  ...pgCapabilities([
    { id: "eq", label: "=", valueKind: "single", placeholder: "key or type" },
    { id: "contains", label: "Contains", valueKind: "single", placeholder: "text" },
  ]),
  kind: "key_value",
  sqlDialect: null,
  queryLanguage: "redis",
  hasSchemas: false,
  structure: "none",
  createTable: false,
  createDatabase: false,
  schemaObjects: false,
  processes: false,
  users: false,
  backup: false,
  import: false,
  diagram: false,
  explain: false,
  metrics: false,
};

const INFO = (id: string): ConnectInfo => ({
  connectionId: id,
  driver: "redis",
  dialect: "postgres",
  database: "db0",
  serverVersion: "7.2.4",
  throughTunnel: false,
});

/** `*` and `?` globs (enough for tests). */
function globMatch(pattern: string, key: string): boolean {
  if (!pattern) return true;
  const re = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${re}$`).test(key);
}

function size(d: Value): number {
  if (d.type === "string") return d.value.length;
  if (d.type === "hash") return Object.keys(d.value).length;
  return d.value.length;
}

function result(sql: string, rows: StatementResult["rows"], column = "value"): StatementResult {
  return {
    sql,
    columns: [{ name: column, typeName: "text" }],
    rows,
    totalRows: rows.length,
    truncated: false,
    rowsAffected: null,
    durationMs: 1,
    error: null,
  };
}

export class FakeRedis {
  keys = new Map<string, Entry>([
    ["user:1", { data: { type: "hash", value: { name: "Ada", age: "36" } }, ttl: -1 }],
    ["user:2", { data: { type: "hash", value: { name: "Alan" } }, ttl: -1 }],
    ["session:abc", { data: { type: "string", value: '{"uid":1}' }, ttl: 300 }],
    ["queue", { data: { type: "list", value: ["a", "b"] }, ttl: -1 }],
  ]);
  edits: KeyEdit[] = [];
  /** Commands the editor ran. */
  ran: string[] = [];

  /** Puts the Redis handlers in front of `backend`, plus a "Local Redis" connection. */
  install(backend: FakeBackend) {
    backend.connections = [
      connectionView({ id: "c-redis", name: "Local Redis", driver: "redis", summary: ":6379" }),
      connectionView({
        id: "c-redis-prod",
        name: "Prod Redis",
        driver: "redis",
        tag: "production",
        summary: "cache:6379",
      }),
      ...backend.connections,
    ];
    const fallback = (cmd: string, args: Args) =>
      (backend as unknown as { handle: (c: string, a: Args) => unknown }).handle(cmd, args);
    mockIPC(
      (cmd, raw) => {
        const args = (raw ?? {}) as Args;
        const own = this.handlers[cmd];
        if (!own) return fallback(cmd, args);
        backend.calls.push({ cmd, args });
        return own(args);
      },
      { shouldMockEvents: true },
    );
  }

  private row(key: string, e: Entry): KeyRow {
    const preview =
      e.data.type === "string" ? e.data.value : JSON.stringify(e.data.value).slice(0, 200);
    return { key, type: e.data.type, ttl: e.ttl, size: size(e.data), preview };
  }

  private detail(key: string): KeyData {
    const e = this.keys.get(key);
    if (!e) return { kind: "missing" };
    const d = e.data;
    switch (d.type) {
      case "string":
        return { kind: "string", text: d.value, binary: null, truncated: false };
      case "hash":
        return {
          kind: "hash",
          entries: Object.entries(d.value).map(([field, value]) => ({ field, value })),
        };
      case "list":
        return { kind: "list", items: d.value.map((value, index) => ({ index, value })) };
      case "set":
        return { kind: "set", members: d.value };
      case "zset":
        return { kind: "zset", members: d.value.map(([member, score]) => ({ member, score })) };
    }
  }

  private apply(e: KeyEdit) {
    const key = "key" in e ? String(e.key) : "";
    const entry = this.keys.get(key);
    switch (e.op) {
      case "hash_set":
        if (entry?.data.type === "hash") {
          if (e.previous !== undefined) delete entry.data.value[String(e.previous)];
          entry.data.value[String(e.field)] = String(e.value);
        }
        break;
      case "expire":
        if (entry) entry.ttl = e.seconds ?? -1;
        break;
      case "delete":
        e.keys.forEach((k) => this.keys.delete(String(k)));
        break;
      case "rename":
        if (entry) {
          this.keys.delete(key);
          this.keys.set(String(e.to), entry);
        }
        break;
      case "set_string":
        if (entry) entry.data = { type: "string", value: e.value };
        break;
      case "create":
        this.keys.set(key, {
          data: { type: "string", value: String(e.value) } as Value,
          ttl: e.ttl ?? -1,
        });
        break;
      default:
        break;
    }
  }

  private run(sql: string): StatementResult[] {
    return sql
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean)
      .map((line) => {
        this.ran.push(line);
        const [cmd = "", key = ""] = line.split(/\s+/);
        if (cmd.toUpperCase() === "GET") {
          const e = this.keys.get(key);
          return result(line, [[e?.data.type === "string" ? e.data.value : null]]);
        }
        if (cmd.toUpperCase() === "DBSIZE") return result(line, [[this.keys.size]]);
        return result(line, [["OK"]]);
      });
  }

  handlers: Record<string, (args: Args) => unknown> = {
    connect: (a) => INFO(a.id),
    driver_capabilities: (a) => (a.driver === "redis" ? REDIS_CAPS : pgCapabilities()),
    redis_databases: (): DatabaseInfo[] => [
      { name: "db0", index: 0, keys: this.keys.size, expires: 1 },
      { name: "db1", index: 1, keys: 0, expires: 0 },
      { name: "db2", index: 2, keys: 7, expires: 0 },
    ],
    redis_scan: (a) => {
      const { pattern, keyType } = a.request;
      const keys = [...this.keys.entries()]
        .filter(([k, e]) => globMatch(pattern, k) && (!keyType || e.data.type === keyType))
        .map(([k, e]) => this.row(k, e));
      return { keys, hasMore: false, total: this.keys.size, query: `SCAN 0 MATCH ${pattern}` };
    },
    redis_read_key: (a) => {
      const key = String(a.request.key);
      const e = this.keys.get(key);
      return {
        key,
        type: e?.data.type ?? "none",
        ttl: e?.ttl ?? -2,
        size: e ? size(e.data) : null,
        encoding: "listpack",
        value: this.detail(key),
        next: "",
      };
    },
    redis_edit: (a) => {
      if (a.id === "c-redis-prod" && !a.confirmed) {
        const statements = [{ sql: "HSET user:1 name Grace", understood: true }];
        const data = { statements, level: "alert" };
        throw { kind: "confirmation_required", layer: "app", message: "confirm", data };
      }
      this.edits.push(a.edit);
      this.apply(a.edit);
      return { affected: 1, statements: [] };
    },
    split_statements: (a) => {
      let offset = 0;
      return String(a.sql)
        .split("\n")
        .map((text) => {
          const st = { text, from: offset, to: offset + text.length };
          offset += text.length + 1;
          return st;
        })
        .filter((st) => st.text.trim());
    },
    run_query: (a) => ({ results: this.run(a.sql), cancelled: false }),
  };
}
