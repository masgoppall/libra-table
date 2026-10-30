// The SQL assistant (src-tauri/src/ai.rs).

export type AiModel = "claude-opus-5-5" | "claude-sonnet-5-5" | "claude-haiku-5-5";

export interface AiRequest {
  question: string;
  /** Dialect and server version, e.g. "postgres 16.2". */
  dialect: string;
  /** "table: col, col" lines. */
  schema: string;
  currentSql?: string;
  model?: AiModel;
}

export interface AiAnswer {
  sql: string;
  note: string;
  model: string;
}
