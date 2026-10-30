// Which app-level overlays are open: palette, settings, shortcuts, "new database",
// backup / restore / command line.

import { type StateCreator } from "zustand";
import { type Group } from "@/types";
import { type AppState } from "../types";

export type PaletteKind = "anything" | "database" | "connection";

/** Backup, restore and "Open in command line" for one database of a connection. */
export interface ServerDialog {
  kind: "backup" | "restore" | "command-line";
  connectionId: string;
  database: string;
}

/** The group dialog of the connections page: a new group (in `parentId`) or `group`. */
export interface GroupEditor {
  group: Group | null;
  parentId: string | null;
}

export interface PaletteState {
  kind: PaletteKind;
  connectionId: string;
}

export interface UiSlice {
  palette: PaletteState | null;
  settingsOpen: boolean;
  shortcutsOpen: boolean;
  backupSchedulesOpen: boolean;
  consoleOpen: boolean;
  /** A passcode is set (the app locks at start and when idle). */
  passcodeEnabled: boolean;
  locked: boolean;
  /** Connection whose "New database" dialog is open. */
  createDatabaseFor: string | null;
  serverDialog: ServerDialog | null;
  /** Bumped to ask the home screen to open the new-connection picker. */
  newConnectionRequest: number;
  /** Uncommitted edits per data tab id, for the toolbar's discard / preview / commit. */
  pendingEdits: Record<string, number>;
  groupEditor: GroupEditor | null;
  openPalette: (kind: PaletteKind, connectionId: string) => void;
  closePalette: () => void;
  setSettingsOpen: (open: boolean) => void;
  setShortcutsOpen: (open: boolean) => void;
  setBackupSchedulesOpen: (open: boolean) => void;
  setConsoleOpen: (open: boolean) => void;
  setPasscodeEnabled: (enabled: boolean) => void;
  setLocked: (locked: boolean) => void;
  setCreateDatabaseFor: (connectionId: string | null) => void;
  setServerDialog: (dialog: ServerDialog | null) => void;
  requestNewConnection: () => void;
  setPendingEdits: (tabId: string, count: number) => void;
  setGroupEditor: (editor: GroupEditor | null) => void;
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  palette: null,
  settingsOpen: false,
  shortcutsOpen: false,
  backupSchedulesOpen: false,
  consoleOpen: false,
  passcodeEnabled: false,
  locked: false,
  createDatabaseFor: null,
  serverDialog: null,
  newConnectionRequest: 0,
  pendingEdits: {},
  groupEditor: null,

  openPalette: (kind, connectionId) => set({ palette: { kind, connectionId } }),
  closePalette: () => set({ palette: null }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
  setBackupSchedulesOpen: (backupSchedulesOpen) => set({ backupSchedulesOpen }),
  setConsoleOpen: (consoleOpen) => set({ consoleOpen }),
  setPasscodeEnabled: (passcodeEnabled) => set({ passcodeEnabled }),
  setLocked: (locked) => set({ locked, palette: null }),
  setCreateDatabaseFor: (createDatabaseFor) => set({ createDatabaseFor }),
  setServerDialog: (serverDialog) => set({ serverDialog }),
  requestNewConnection: () =>
    set((s) => ({ view: "home", newConnectionRequest: s.newConnectionRequest + 1 })),
  setPendingEdits: (tabId, count) =>
    set((s) => {
      if ((s.pendingEdits[tabId] ?? 0) === count) return s;
      const pendingEdits = { ...s.pendingEdits };
      if (count) pendingEdits[tabId] = count;
      else delete pendingEdits[tabId];
      return { pendingEdits };
    }),
  setGroupEditor: (groupEditor) => set({ groupEditor }),
});
