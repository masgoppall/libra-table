// Metrics boards per connection, and widget queries (read-only, variables filled in Rust).

import { type Db, type MetricBoard, type MetricVariable, type StatementResult } from "@/types";
import { call } from "./client";

export const metricsApi = {
  listMetricBoards: (id: string) => call<MetricBoard[]>("list_metric_boards", { id }),
  saveMetricBoard: (id: string, board: MetricBoard) =>
    call<MetricBoard>("save_metric_board", { id, board }),
  deleteMetricBoard: (id: string, boardId: string) =>
    call<void>("delete_metric_board", { id, boardId }),
  runMetricQuery: (id: string, db: Db, sql: string, variables: MetricVariable[]) =>
    call<StatementResult>("run_metric_query", { id, database: db, sql, variables }),
};
