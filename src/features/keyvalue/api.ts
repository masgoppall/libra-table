// Typed wrappers around the Redis key-view commands.

import { call } from "@/api/client";
import { type Db } from "@/types";
import {
  type DatabaseInfo,
  type EditResult,
  type KeyDetail,
  type KeyEdit,
  type RedisValue,
  type ScanPage,
  type ScanRequest,
} from "./types";

export const redisApi = {
  databases: (id: string) => call<DatabaseInfo[]>("redis_databases", { id }),
  scan: (id: string, db: Db, request: ScanRequest) =>
    call<ScanPage>("redis_scan", { id, database: db, request }),
  readKey: (id: string, db: Db, key: RedisValue, cursor = "", count = 200) =>
    call<KeyDetail>("redis_read_key", { id, database: db, request: { key, cursor, count } }),
  edit: (id: string, db: Db, edit: KeyEdit, confirmed: boolean) =>
    call<EditResult>("redis_edit", { id, database: db, edit, confirmed }),
};
