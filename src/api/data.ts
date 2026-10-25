// Table pages, row counts, filter previews and committed edits.

import {
  type CellContent,
  type CellTarget,
  type Change,
  type CommitResult,
  type Db,
  type FilterPreview,
  type PageRequest,
  type PageResult,
  type RowCount,
} from "@/types";
import { call } from "./client";

export const dataApi = {
  fetchPage: (id: string, db: Db, request: PageRequest) =>
    call<PageResult>("fetch_page", { id, database: db, request }),
  countRows: (id: string, db: Db, request: PageRequest) =>
    call<RowCount>("count_rows", { id, database: db, request }),
  previewFilter: (id: string, db: Db, request: PageRequest) =>
    call<FilterPreview>("preview_filter", { id, database: db, request }),
  previewChanges: (id: string, db: Db, schema: string, table: string, changes: Change[]) =>
    call<string[]>("preview_changes", { id, database: db, schema, table, changes }),
  fetchCell: (id: string, db: Db, target: CellTarget) =>
    call<CellContent>("fetch_cell", { id, database: db, ...target }),
  columnChoices: (id: string, db: Db, schema: string, table: string) =>
    call<Record<string, string[]>>("column_choices", { id, database: db, schema, table }),
  commitChanges: (
    id: string,
    db: Db,
    target: { schema: string; table: string },
    changes: Change[],
    confirmed: boolean,
  ) =>
    call<CommitResult>("commit_changes", {
      id,
      database: db,
      ...target,
      changes,
      confirmed,
    }),
};
