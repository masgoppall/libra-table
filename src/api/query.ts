// Running SQL, saved queries and query history.

import {
  type Db,
  type Driver,
  type HistoryEntry,
  type HistoryQuery,
  type RunResponse,
  type SavedQuery,
  type Statement,
} from "@/types";
import { call } from "./client";

export const queryApi = {
  splitStatements: (driver: Driver, sql: string) =>
    call<Statement[]>("split_statements", { driver, sql }),
  runQuery: (id: string, db: Db, sql: string, queryId: string, confirmed: boolean) =>
    call<RunResponse>("run_query", { id, database: db, sql, queryId, confirmed }),
  cancelQuery: (queryId: string) => call<boolean>("cancel_query", { queryId }),
};

export const historyApi = {
  searchHistory: (query: HistoryQuery) => call<HistoryEntry[]>("search_history", { query }),
  deleteHistory: (ids: number[]) => call<number>("delete_history", { ids }),
  purgeHistory: (olderThanDays: number) => call<number>("purge_history", { olderThanDays }),
};

export const savedQueriesApi = {
  listSavedQueries: (id: string) => call<SavedQuery[]>("list_saved_queries", { id }),
  readSavedQuery: (id: string, name: string) => call<string>("read_saved_query", { id, name }),
  writeSavedQuery: (id: string, name: string, sql: string) =>
    call<void>("write_saved_query", { id, name, sql }),
  renameSavedQuery: (id: string, from: string, to: string) =>
    call<void>("rename_saved_query", { id, from, to }),
  deleteSavedQuery: (id: string, name: string) => call<void>("delete_saved_query", { id, name }),
};
