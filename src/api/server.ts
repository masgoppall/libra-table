// Server tools: process list, users & privileges, backup / restore ("tool-progress"
// events), command line.

import {
  type BackupRequest,
  type CommandLine,
  type Db,
  type Grantee,
  type ProcessAction,
  type ProcessInfo,
  type RestoreRequest,
  type ToolPlan,
  type UserChange,
  type UserList,
} from "@/types";
import { call } from "./client";

export const serverApi = {
  listProcesses: (id: string) => call<ProcessInfo[]>("list_processes", { id }),
  previewProcessAction: (id: string, processId: number, action: ProcessAction) =>
    call<string[]>("preview_process_action", { id, processId, action }),
  runProcessAction: (id: string, processId: number, action: ProcessAction, confirmed: boolean) =>
    call<void>("run_process_action", { id, processId, action, confirmed }),

  listUsers: (id: string) => call<UserList>("list_users", { id }),
  userGrants: (id: string, db: Db, grantee: Grantee) =>
    call<string[]>("user_grants", { id, database: db, grantee }),
  previewUserChange: (id: string, db: Db, change: UserChange) =>
    call<string[]>("preview_user_change", { id, database: db, change }),
  applyUserChange: (id: string, db: Db, change: UserChange, confirmed: boolean) =>
    call<string[]>("apply_user_change", { id, database: db, change, confirmed }),
};

export const toolsApi = {
  planBackup: (id: string, db: Db, request: BackupRequest) =>
    call<ToolPlan>("plan_backup", { id, database: db, request }),
  planRestore: (id: string, db: Db, request: RestoreRequest) =>
    call<ToolPlan>("plan_restore", { id, database: db, request }),
  setToolPath: (tool: string, path: string | null) => call<void>("set_tool_path", { tool, path }),
  startBackup: (id: string, db: Db, jobId: string, request: BackupRequest) =>
    call<void>("start_backup", { id, database: db, jobId, request }),
  startRestore: (id: string, db: Db, jobId: string, request: RestoreRequest, confirmed: boolean) =>
    call<void>("start_restore", { id, database: db, jobId, request, confirmed }),
  cancelToolJob: (jobId: string) => call<boolean>("cancel_tool_job", { jobId }),
  commandLine: (id: string, db: Db) => call<CommandLine>("command_line", { id, database: db }),
  openInTerminal: (id: string, db: Db) => call<void>("open_in_terminal", { id, database: db }),
};
