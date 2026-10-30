// App settings and info, plus the open-tabs file restored on the next start (STO-5).

import { type AiModel } from "./ai";
import { type WorkspaceTab } from "./workspace";

export type Theme = "system" | "light" | "dark";
export type DefaultFilterColumn = "first" | "any" | "pk";
export type RowDensity = "compact" | "normal" | "comfortable";
export type EditorKeymap = "default" | "vim";
/** How query variables are written: `:name`, `$name`, `${name}`, `{{name}}`, `%name%`. */
export type VariableFormat = "colon" | "dollar" | "dollar-braces" | "double-braces" | "percent";

export type ThemeSlot =
  | "window"
  | "content"
  | "sidebar"
  | "tabs"
  | "tabSelected"
  | "tabHover"
  | "tabText"
  | "tabSelectedText"
  | "text"
  | "textDim"
  | "border"
  | "accent"
  | "selection"
  | "gridOdd"
  | "gridEven"
  | "gridHeader"
  | "gridLine"
  | "inserted"
  | "modified"
  | "deleted";

/** A color theme: a light or dark base and "#rrggbb" colors for some slots. */
export interface ColorTheme {
  id: string;
  name: string;
  base: "light" | "dark";
  colors: Partial<Record<ThemeSlot, string>>;
}

/** A backup that runs on a schedule while LibraTable is open. */
export interface BackupSchedule {
  id: string;
  name: string;
  connectionId: string;
  databases: string[];
  folder: string;
  every: "hourly" | "daily" | "weekly";
  /** "HH:MM"; hourly runs at its minute. */
  at: string;
  /** 0 = Sunday … 6 = Saturday. */
  weekday: number;
  /** Backups kept per database (0 keeps all). */
  keep: number;
  enabled: boolean;
  lastRun: number | null;
  lastError: string | null;
}

/** A custom connection tag: a name and a "#rrggbb" color. */
export interface CustomTag {
  name: string;
  color: string;
}

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
  /** Tables open in a tab of their own; off, they replace the current table tab. */
  openInNewTab: boolean;
  alternatingRows: boolean;
  showRowNumbers: boolean;
  rowDensity: RowDensity;
  editorKeymap: EditorKeymap;
  /** SQL keywords become UPPERCASE as they are typed. */
  uppercaseKeywords: boolean;
  /** Typing a bracket or quote adds the closing one. */
  closeBrackets: boolean;
  customTags: CustomTag[];
  variableFormat: VariableFormat;
  /** Variables inside quoted strings count too. */
  variablesInStrings: boolean;
  /** Id of the color theme ("" for LibraTable's own light and dark). */
  colorTheme: string;
  /** Themes imported from files. */
  customThemes: ColorTheme[];
  /** The SQL assistant is on: the user agreed to send the schema and questions to it. */
  aiEnabled: boolean;
  aiModel: AiModel;
  backupSchedules: BackupSchedule[];
}

export interface AppInfo {
  version: string;
  dataDir: string;
}

export interface SavedWorkspace {
  database?: string;
  databases?: string[];
  schema: string;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
}

export interface WorkspaceFile {
  connections?: Record<string, SavedWorkspace>;
  /** Connection ids open when the app quit. */
  open?: string[];
}
