import { type ConnectionsSlice } from "./slices/connections";
import { type DialogsSlice } from "./slices/dialogs";
import { type SettingsSlice } from "./slices/settings";
import { type ToastsSlice } from "./slices/toasts";
import { type ToolsSlice } from "./slices/tools";
import { type UiSlice } from "./slices/ui";
import { type WorkspacesSlice } from "./slices/workspaces";

export type AppState = ConnectionsSlice &
  SettingsSlice &
  WorkspacesSlice &
  UiSlice &
  DialogsSlice &
  ToastsSlice &
  ToolsSlice;
