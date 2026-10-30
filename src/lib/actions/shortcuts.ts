// Every shortcut is an action; the native menu (src-tauri/src/menu.rs) sends the same ids.

export type TabNumberAction = `tab-${number}`;

export type ActionId =
  | "new-connection"
  | "new-query"
  | "new-table"
  | "new-database"
  | "save"
  | "export"
  | "close-tab"
  | "disconnect"
  | "reconnect"
  | "reconnect-all"
  | "refresh"
  | "toggle-sidebar"
  | "toggle-detail"
  | "toggle-console"
  | "font-bigger"
  | "font-smaller"
  | "font-reset"
  | "filter"
  | "add-row"
  | "duplicate-row"
  | "jump-to"
  | "find-replace"
  | "discard"
  | "undo"
  | "prev-page"
  | "next-page"
  | "open-structure"
  | "run-current"
  | "run-all"
  | "cancel-query"
  | "explain"
  | "format-sql"
  | "trim-whitespace"
  | "ask-ai"
  | "history"
  | "open-anything"
  | "switch-database"
  | "open-connection"
  | "new-window"
  | "next-tab"
  | "prev-tab"
  | "tab-back"
  | "tab-forward"
  | "home"
  | "settings"
  | "shortcuts"
  | "process-list"
  | "users"
  | "backup"
  | "restore"
  | "command-line"
  | "lock"
  | "import-data"
  | "import-new-table"
  | "import-sql"
  | "export-tables"
  | "open-diagram"
  | "open-metrics"
  | "compare-schema"
  | "backup-schedules"
  | TabNumberAction;

export type ShortcutGroup = "General" | "Tabs" | "Data" | "Query" | "View";

export interface Shortcut {
  id: ActionId;
  label: string;
  /** "Mod" is ⌘ on macOS, Ctrl elsewhere. */
  keys: string;
  group: ShortcutGroup;
}

export const SHORTCUT_GROUPS: ShortcutGroup[] = ["General", "Tabs", "Data", "Query", "View"];

const general = (id: ActionId, label: string, keys: string): Shortcut => ({
  id,
  label,
  keys,
  group: "General",
});

export const SHORTCUTS: Shortcut[] = [
  general("open-anything", "Open table, view or query", "Mod+P"),
  general("switch-database", "Open database", "Mod+K"),
  general("open-connection", "Open another connection", "Mod+Shift+K"),
  general("new-window", "This connection in a new window", "Mod+Alt+N"),
  general("new-connection", "New connection", "Mod+N"),
  general("new-table", "New table", "Mod+Shift+N"),
  general("save", "Save query / commit changes / apply structure", "Mod+S"),
  general("refresh", "Refresh", "Mod+R"),
  general("export", "Export", "Mod+Shift+E"),
  general("settings", "Settings", "Mod+,"),
  general("shortcuts", "Keyboard shortcuts", "Mod+/"),
  general("home", "Connections", "Mod+Shift+H"),
  general("disconnect", "Disconnect", "Mod+Shift+W"),
  general("reconnect", "Reconnect", "Mod+Shift+R"),
  general("lock", "Lock LibraTable", "Mod+Shift+L"),

  { id: "new-query", label: "New query tab", keys: "Mod+T", group: "Tabs" },
  { id: "close-tab", label: "Close tab", keys: "Mod+W", group: "Tabs" },
  { id: "next-tab", label: "Next tab", keys: "Mod+Shift+]", group: "Tabs" },
  { id: "prev-tab", label: "Previous tab", keys: "Mod+Shift+[", group: "Tabs" },
  { id: "next-tab", label: "Next tab", keys: "Ctrl+Tab", group: "Tabs" },
  { id: "prev-tab", label: "Previous tab", keys: "Ctrl+Shift+Tab", group: "Tabs" },
  { id: "tab-1", label: "Go to tab 1…9", keys: "Mod+1", group: "Tabs" },
  { id: "tab-back", label: "Back (what the tab showed before)", keys: "Mod+Alt+[", group: "Tabs" },
  { id: "tab-forward", label: "Forward", keys: "Mod+Alt+]", group: "Tabs" },
  { id: "history", label: "Query history", keys: "Mod+Y", group: "Tabs" },

  { id: "filter", label: "Filter rows", keys: "Mod+F", group: "Data" },
  { id: "add-row", label: "Add row", keys: "Mod+I", group: "Data" },
  { id: "duplicate-row", label: "Duplicate selected rows", keys: "Mod+D", group: "Data" },
  { id: "jump-to", label: "Jump to column or row", keys: "Mod+J", group: "Data" },
  {
    id: "find-replace",
    label: "Search and replace within table",
    keys: "Mod+Alt+R",
    group: "Data",
  },
  { id: "undo", label: "Undo last edit", keys: "Mod+Z", group: "Data" },
  { id: "prev-page", label: "Previous page", keys: "Mod+[", group: "Data" },
  { id: "next-page", label: "Next page", keys: "Mod+]", group: "Data" },
  { id: "open-structure", label: "Show structure", keys: "Mod+Shift+S", group: "Data" },
  { id: "toggle-detail", label: "Row detail panel", keys: "Mod+Shift+D", group: "Data" },

  {
    id: "run-current",
    label: "Run statement at cursor (or selection)",
    keys: "Mod+Enter",
    group: "Query",
  },
  { id: "run-all", label: "Run all statements", keys: "Mod+Shift+Enter", group: "Query" },
  { id: "cancel-query", label: "Cancel running query", keys: "Mod+.", group: "Query" },
  // ⌘I is "Add row" here, so Beautify (⌘I in TablePlus) moves to ⌥⌘F.
  { id: "explain", label: "Explain statement at cursor", keys: "Mod+Alt+E", group: "Query" },
  { id: "format-sql", label: "Beautify SQL (selection or all)", keys: "Mod+Alt+F", group: "Query" },
  { id: "ask-ai", label: "Ask the SQL assistant", keys: "Mod+Shift+I", group: "Query" },

  { id: "toggle-sidebar", label: "Toggle sidebar", keys: "Mod+\\", group: "View" },
  { id: "toggle-console", label: "Console log", keys: "Mod+Shift+C", group: "View" },
  { id: "font-bigger", label: "Larger text", keys: "Mod+=", group: "View" },
  { id: "font-smaller", label: "Smaller text", keys: "Mod+-", group: "View" },
  { id: "font-reset", label: "Actual size", keys: "Mod+0", group: "View" },
];

/** Actions without a default key (menu and palette only), with their labels. */
export const UNBOUND_ACTIONS: Partial<Record<ActionId, string>> = {
  "process-list": "Process list",
  users: "Users & privileges",
  backup: "Backup…",
  restore: "Restore…",
  "command-line": "Open in command line",
  "import-data": "Import CSV or JSON…",
  "import-new-table": "Import into new table…",
  "import-sql": "Run SQL file…",
  "export-tables": "Export tables…",
  "open-diagram": "ER diagram",
  "open-metrics": "Metrics",
  "trim-whitespace": "Trim trailing whitespace",
  "compare-schema": "Compare schema…",
  "backup-schedules": "Backup schedules…",
  "reconnect-all": "Reconnect all connections",
};

export const tabAction = (n: number): TabNumberAction => `tab-${n}`;

/** Extra keys that only the UI handles (not listed separately). */
export const EXTRA_BINDINGS: { id: ActionId; keys: string }[] = [
  { id: "refresh", keys: "F5" },
  { id: "font-bigger", keys: "Mod+Shift+=" },
  ...Array.from({ length: 9 }, (_, i) => ({ id: tabAction(i + 1), keys: `Mod+${i + 1}` })),
];

/** Grid and editor keys that aren't app actions, for the shortcuts sheet. */
export const NON_ACTION_KEYS: Partial<Record<ShortcutGroup, [string, string][]>> = {
  Data: [
    ["↵ / F2", "Edit the focused cell (or just start typing)"],
    ["⇥ / ⇧⇥", "Next / previous cell"],
    ["⌫", "Delete selected rows"],
    ["⌘C", "Copy selected rows (or the selected cells)"],
    ["⌘A", "Select all rows on the page"],
    ["⇧↑ / ⇧↓", "Extend the selection"],
    ["⇧← / ⇧→", "Select a block of cells (or drag across cells)"],
    ["⌘V", "Paste rows, a value or a block of cells"],
  ],
  Query: [
    ["⌘/", "Comment / uncomment lines"],
    ["⌥⌘/", "Block comment / uncomment"],
    ["⌘] / ⌘[", "Shift lines right / left"],
    ["⌘F", "Find in the editor"],
    ["⌃Space", "Autocomplete"],
  ],
};
