// Fake commands for imports, SQL files, multi-table exports, the ER diagram and
// metrics boards. Jobs answer with the events Rust would send.

import { emit } from "@tauri-apps/api/event";
import {
  type FilePreview,
  type MetricBoard,
  type SchemaDiagram,
  type StatementResult,
} from "@/types";
import { type Args } from "./fakeBackend";

export const PREVIEW: FilePreview = {
  columns: [
    { name: "ID", kind: "integer", suggestedType: "integer", nullable: false },
    { name: "name", kind: "text", suggestedType: "text", nullable: false },
    { name: "Note", kind: "text", suggestedType: "text", nullable: true },
  ],
  rows: [
    ["1", "Ann", null],
    ["2", "Bob", "hi"],
  ],
  sampled: 2,
  totalBytes: 64,
  errors: [],
};

export const DIAGRAM: SchemaDiagram = {
  schema: "public",
  truncated: false,
  tables: [
    {
      name: "orders",
      columns: [
        { name: "id", dataType: "integer", nullable: false, primaryKey: true },
        { name: "person_id", dataType: "integer", nullable: false, primaryKey: false },
      ],
      foreignKeys: [
        {
          name: "orders_person",
          columns: ["person_id"],
          refSchema: "public",
          refTable: "people",
          refColumns: ["id"],
        },
      ],
    },
    {
      name: "people",
      columns: [
        { name: "id", dataType: "integer", nullable: false, primaryKey: true },
        { name: "name", dataType: "text", nullable: false, primaryKey: false },
      ],
      foreignKeys: [],
    },
  ],
};

function result(columns: string[], rows: StatementResult["rows"]): StatementResult {
  return {
    sql: "",
    columns: columns.map((name) => ({ name, typeName: "" })),
    rows,
    totalRows: rows.length,
    truncated: false,
    rowsAffected: null,
    durationMs: 1,
    error: null,
  };
}

/** What Rust answers for an unconfirmed write on a production connection. */
function needsConfirmation() {
  const statements = [{ sql: "INSERT INTO … VALUES ($1, …)", understood: true }];
  return { kind: "confirmation_required", layer: "app", message: "confirm", data: { statements } };
}

/** Sends a job's final event after the command returned. */
function finishLater(event: string, payload: unknown) {
  setTimeout(() => void emit(event, payload), 10);
}

export class FakeTools {
  /** What the file / folder picker returns. */
  pickedPath = "/tmp/people.csv";
  boards: MetricBoard[] = [];
  layouts: Record<string, unknown> = {};

  handlers: Record<string, (a: Args) => unknown> = {
    "plugin:dialog|open": () => this.pickedPath,
    "plugin:dialog|save": () => null,
    preview_import_file: () => PREVIEW,
    preview_import: (a) => [`-- import into ${a.request.table}`, "INSERT INTO … VALUES ($1, …)"],
    start_import: (a) => {
      if (a.id === "c-prod" && !a.confirmed) throw needsConfirmation();
      const rows = PREVIEW.rows.length;
      const summary = { inserted: rows, skipped: 0, rowsRead: rows, errors: [], errorCount: 0 };
      const done = { ...summary, cancelled: false, stopped: false, rolledBack: false, fatal: null };
      const progress = {
        rowsRead: rows,
        inserted: rows,
        skipped: 0,
        bytesRead: 64,
        totalBytes: 64,
      };
      finishLater("import-progress", { jobId: a.jobId, progress, done: true, summary: done });
      return null;
    },
    start_sql_file: () => null,
    start_export_tables: () => null,
    cancel_import: () => true,
    schema_diagram: () => DIAGRAM,
    get_diagram_layout: (a) => this.layouts[a.key] ?? {},
    save_diagram_layout: (a) => {
      this.layouts[a.key] = a.layout;
      return null;
    },
    list_metric_boards: () => this.boards,
    save_metric_board: (a) => {
      const b = a.board as MetricBoard;
      this.boards = [...this.boards.filter((x) => x.id !== b.id), b];
      return b;
    },
    delete_metric_board: (a) => {
      this.boards = this.boards.filter((x) => x.id !== a.boardId);
      return null;
    },
    run_metric_query: (a) =>
      /count/i.test(a.sql)
        ? result(["n"], [[42]])
        : result(
            ["month", "sales"],
            [
              ["Jan", 10],
              ["Feb", 30],
            ],
          ),
  };
}
