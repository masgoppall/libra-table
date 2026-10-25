// Global UI state (zustand). Data lives in Rust; this holds what is on screen.
// Each slice owns one concern; components read it with selectors (./selectors).

import { create } from "zustand";
import { createConnectionsSlice } from "./slices/connections";
import { createDialogsSlice } from "./slices/dialogs";
import { createSettingsSlice } from "./slices/settings";
import { createToastsSlice } from "./slices/toasts";
import { createToolsSlice } from "./slices/tools";
import { createUiSlice } from "./slices/ui";
import { createWorkspacesSlice } from "./slices/workspaces";
import { type AppState } from "./types";

export const useStore = create<AppState>()((...a) => ({
  ...createConnectionsSlice(...a),
  ...createSettingsSlice(...a),
  ...createWorkspacesSlice(...a),
  ...createUiSlice(...a),
  ...createDialogsSlice(...a),
  ...createToastsSlice(...a),
  ...createToolsSlice(...a),
}));

export type { AppState } from "./types";
export type { DialogResult, DialogSpec, OpenDialog } from "./slices/dialogs";
export type { Toast, ToastKind } from "./slices/toasts";
export type { ToolDialog } from "./slices/tools";
export type { PaletteKind, PaletteState, ServerDialog } from "./slices/ui";
export { HOME } from "./slices/workspaces";
export * from "./selectors";
export { activeTab, tabDb } from "./workspaceModel";
