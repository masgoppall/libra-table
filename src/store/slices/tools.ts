// Which workspace tool dialog is open: import wizard, SQL file runner, multi-table export.

import { type StateCreator } from "zustand";
import { type Db } from "@/types";
import { type AppState } from "../types";

interface ToolTarget {
  connectionId: string;
  /** Database and schema the dialog works in (the sidebar's, when it opened). */
  db: Db;
  schema: string;
}

export type ToolDialog =
  /** `table` null imports into a new table. */
  | (ToolTarget & { kind: "import"; table: string | null })
  | (ToolTarget & { kind: "sql-file" })
  | (ToolTarget & { kind: "export-tables"; selected: string[] })
  | (ToolTarget & { kind: "compare-schema" });

export interface ToolsSlice {
  toolDialog: ToolDialog | null;
  openToolDialog: (dialog: ToolDialog) => void;
  closeToolDialog: () => void;
}

export const createToolsSlice: StateCreator<AppState, [], [], ToolsSlice> = (set) => ({
  toolDialog: null,
  openToolDialog: (toolDialog) => set({ toolDialog }),
  closeToolDialog: () => set({ toolDialog: null }),
});
