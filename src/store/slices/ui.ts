// Which app-level overlays are open: palette, settings, shortcuts, "new database",
// backup / restore / command line.

import { type StateCreator } from "zustand";
import { type AppState } from "../types";

export type PaletteKind = "anything" | "database";

/** Backup, restore and "Open in command line" for one database of a connection. */
export interface ServerDialog {
  kind: "backup" | "restore" | "command-line";
  connectionId: string;
  database: string;
}

export interface PaletteState {
  kind: PaletteKind;
  connectionId: string;
}

export interface UiSlice {
  palette: PaletteState | null;
  settingsOpen: boolean;
  shortcutsOpen: boolean;
  consoleOpen: boolean;
  /** A passcode is set (the app locks at start and when idle). */
  passcodeEnabled: boolean;
  locked: boolean;
  /** Connection whose "New database" dialog is open. */
  createDatabaseFor: string | null;
  serverDialog: ServerDialog | null;
  /** Bumped to ask the home screen to open the new-connection picker. */
  newConnectionRequest: number;
  openPalette: (kind: PaletteKind, connectionId: string) => void;
  closePalette: () => void;
  setSettingsOpen: (open: boolean) => void;
  setShortcutsOpen: (open: boolean) => void;
  setConsoleOpen: (open: boolean) => void;
  setPasscodeEnabled: (enabled: boolean) => void;
  setLocked: (locked: boolean) => void;
  setCreateDatabaseFor: (connectionId: string | null) => void;
  setServerDialog: (dialog: ServerDialog | null) => void;
  requestNewConnection: () => void;
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  palette: null,
  settingsOpen: false,
  shortcutsOpen: false,
  consoleOpen: false,
  passcodeEnabled: false,
  locked: false,
  createDatabaseFor: null,
  serverDialog: null,
  newConnectionRequest: 0,

  openPalette: (kind, connectionId) => set({ palette: { kind, connectionId } }),
  closePalette: () => set({ palette: null }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
  setConsoleOpen: (consoleOpen) => set({ consoleOpen }),
  setPasscodeEnabled: (passcodeEnabled) => set({ passcodeEnabled }),
  setLocked: (locked) => set({ locked, palette: null }),
  setCreateDatabaseFor: (createDatabaseFor) => set({ createDatabaseFor }),
  setServerDialog: (serverDialog) => set({ serverDialog }),
  requestNewConnection: () =>
    set((s) => ({ view: "home", newConnectionRequest: s.newConnectionRequest + 1 })),
});
