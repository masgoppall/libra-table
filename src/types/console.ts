// Console log entries: every statement the app sent, as reported by the database driver.

export interface ConsoleEntry {
  at: number;
  sql: string;
  rowsAffected: number;
  rowsReturned: number;
  elapsedMs: number;
  /** Slower than one second. */
  slow: boolean;
}
