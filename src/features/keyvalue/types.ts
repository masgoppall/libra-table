// What the Redis commands (src-tauri/src/commands/redis.rs) send and take.

import { type Cell } from "@/types";

/** A key, field or member as Rust sends it: text, or a binary preview. */
export type RedisValue = Cell;

export type KeyType = "string" | "hash" | "list" | "set" | "zset" | "stream";

export const KEY_TYPES: KeyType[] = ["string", "hash", "list", "set", "zset", "stream"];

export interface DatabaseInfo {
  name: string;
  index: number;
  keys: number;
  expires: number;
}

export interface KeyRow {
  key: RedisValue;
  /** string, hash… or a module type (ReJSON-RL…). */
  type: string;
  /** Seconds; -1 = no expiry. */
  ttl: number;
  size: number | null;
  preview: RedisValue;
}

export interface ScanRequest {
  pattern: string;
  keyType: string;
  page: number;
  pageSize: number;
}

export interface ScanPage {
  keys: KeyRow[];
  hasMore: boolean;
  /** Every key of the database (DBSIZE). */
  total: number;
  query: string;
}

export interface HashEntry {
  field: RedisValue;
  value: RedisValue;
}

export interface ListItem {
  index: number;
  value: RedisValue;
}

export interface ZsetMember {
  member: RedisValue;
  score: number | string;
}

export interface StreamEntry {
  id: string;
  fields: [RedisValue, RedisValue][];
}

export type KeyData =
  | { kind: "string"; text: string | null; binary: RedisValue | null; truncated: boolean }
  | { kind: "hash"; entries: HashEntry[] }
  | { kind: "list"; items: ListItem[] }
  | { kind: "set"; members: RedisValue[] }
  | { kind: "zset"; members: ZsetMember[] }
  | { kind: "stream"; entries: StreamEntry[] }
  | { kind: "other" }
  | { kind: "missing" };

export interface KeyDetail {
  key: RedisValue;
  type: string;
  ttl: number;
  size: number | null;
  encoding: string | null;
  value: KeyData;
  /** Cursor of the next page; "" when this was the last. */
  next: string;
}

type K = { key: RedisValue };

/** One change to a key (`KeyEdit` in Rust). */
export type KeyEdit =
  | (K & { op: "set_string"; value: string })
  | (K & { op: "hash_set"; field: RedisValue; value: RedisValue; previous?: RedisValue })
  | (K & { op: "hash_delete"; fields: RedisValue[] })
  | (K & { op: "list_set"; index: number; value: RedisValue; expected: RedisValue })
  | (K & { op: "list_push"; values: RedisValue[]; head: boolean })
  | (K & { op: "list_remove"; index: number; expected: RedisValue })
  | (K & { op: "set_add" | "set_remove"; members: RedisValue[] })
  | (K & { op: "zset_add"; member: RedisValue; score: number; previous?: RedisValue })
  | (K & { op: "zset_remove"; members: RedisValue[] })
  | (K & { op: "expire"; seconds: number | null })
  | (K & { op: "rename"; to: RedisValue })
  | { op: "delete"; keys: RedisValue[] }
  | (K & { op: "create"; type: KeyType; value: unknown; ttl: number | null });

export interface EditResult {
  affected: number;
  statements: string[];
}
