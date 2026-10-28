// Settings, app info and per-user UI state kept by Rust.

import {
  type AppInfo,
  type ConsoleEntry,
  type Settings,
  type TableState,
  type WorkspaceFile,
} from "@/types";
import { call } from "./client";

export const appApi = {
  appInfo: () => call<AppInfo>("app_info"),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  getWorkspace: () => call<WorkspaceFile>("get_workspace"),
  saveWorkspace: (workspace: WorkspaceFile) => call<void>("save_workspace", { workspace }),
  passcodeSet: () => call<boolean>("passcode_set"),
  verifyPasscode: (code: string) => call<boolean>("verify_passcode", { code }),
  changePasscode: (current: string | null, next: string | null) =>
    call<void>("change_passcode", { current, next }),
  consoleEntries: () => call<ConsoleEntry[]>("console_entries"),
  clearConsole: () => call<void>("clear_console"),
  setWindowView: (inWorkspace: boolean) => call<void>("set_window_view", { inWorkspace }),
  setWindowMode: (mode: "welcome" | "workspace") => call<void>("set_window_mode", { mode }),
  /** An image file as a `data:` URL (the group icon picker). */
  readImageFile: (path: string) => call<string>("read_image_file", { path }),
  getTableState: (key: string) => call<TableState | null>("get_table_state", { key }),
  saveTableState: (key: string, tableState: TableState) =>
    call<void>("save_table_state", { key, tableState }),
};
