// App settings and info, plus the open-tabs file restored on the next start (STO-5).

import { type WorkspaceTab } from "./workspace";

export type Theme = "system" | "light" | "dark";
export type DefaultFilterColumn = "first" | "any" | "pk";

export interface Settings {
  historyRetentionDays: number;
  defaultFilterOperator: string;
  defaultFilterColumn: DefaultFilterColumn;
  theme: Theme;
  editorFontSize: number;
  uiFontSize: number;
  /** With a passcode set, lock after this many idle minutes (0 = only at start). */
  lockAfterMinutes: number;
  /** Custom keyboard shortcuts: action id → keys ("Mod+Shift+K"). */
  shortcuts: Record<string, string>;
}

export interface AppInfo {
  version: string;
  dataDir: string;
}

export interface SavedWorkspace {
  database?: string;
  schema: string;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
}

export interface WorkspaceFile {
  connections?: Record<string, SavedWorkspace>;
  /** Connection ids open when the app quit. */
  open?: string[];
}
