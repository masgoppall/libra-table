// App settings, loaded once and saved through Rust (which validates them).

import { type StateCreator } from "zustand";
import { api } from "@/api";
import { type Settings } from "@/types";
import { type AppState } from "../types";

export interface SettingsSlice {
  settings: Settings | null;
  loadSettings: () => Promise<void>;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
}

export const createSettingsSlice: StateCreator<AppState, [], [], SettingsSlice> = (set, get) => ({
  settings: null,

  loadSettings: async () => set({ settings: await api.getSettings() }),

  saveSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    set({ settings: await api.saveSettings({ ...current, ...patch }) });
  },
});
