// The app shell: toolbar, the connections page or a workspace, and global overlays.

import { useEffect } from "react";
import { DialogHost } from "@/components/dialogs/DialogHost";
import { Toasts } from "@/components/Toasts";
import { Home } from "@/features/connections/Home";
import { LockScreen } from "@/features/lock/LockScreen";
import { BackupSchedulesDialog } from "@/features/server/backup/BackupSchedulesDialog";
import { usePasscodeLock } from "@/features/lock/usePasscodeLock";
import { ConsolePanel } from "@/features/console/ConsolePanel";
import { Palette } from "@/features/palette/Palette";
import { SchemaDialogHost } from "@/features/schema/SchemaDialogHost";
import { ServerDialogHost } from "@/features/server/ServerDialogHost";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { ShortcutsDialog } from "@/features/settings/ShortcutsDialog";
import { WorkspaceView } from "@/features/workspace/WorkspaceView";
import { showError } from "@/services/errors";
import { HOME, useStore } from "@/store";
import { ConnectionTabs } from "./toolbar/ConnectionTabs";
import { Toolbar } from "./toolbar/Toolbar";
import { useAppActions } from "./useAppActions";
import { GridPrefsContext } from "@/components/grid/prefs";

import { useBackupScheduler } from "./useBackupScheduler";
import { useConnectionWindow } from "./useConnectionWindow";
import { useAppearance, useGridPrefsFromSettings } from "./useAppearance";
import { useCustomShortcuts, useGlobalKeys } from "./useGlobalKeys";
import { useWindowClose } from "./useWindowClose";
import { useWindowMode } from "./useWindowMode";

function Overlays() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const shortcutsOpen = useStore((s) => s.shortcutsOpen);
  const schedulesOpen = useStore((s) => s.backupSchedulesOpen);
  const { setSettingsOpen, setShortcutsOpen } = useStore.getState();
  return (
    <>
      <Palette />
      <SchemaDialogHost />
      <ServerDialogHost />
      {settingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} />}
      {shortcutsOpen && <ShortcutsDialog onClose={() => setShortcutsOpen(false)} />}
      {schedulesOpen && (
        <BackupSchedulesDialog onClose={() => useStore.getState().setBackupSchedulesOpen(false)} />
      )}
      <DialogHost />
      <Toasts />
    </>
  );
}

export default function App() {
  const view = useStore((s) => s.view);
  const workspaces = useStore((s) => s.workspaces);
  const consoleOpen = useStore((s) => s.consoleOpen);

  useEffect(() => {
    const { loadConnections, loadDrivers, loadSettings } = useStore.getState();
    loadConnections().catch(showError);
    loadDrivers().catch(showError);
    loadSettings().catch(showError);
  }, []);
  useAppearance();
  useCustomShortcuts();
  usePasscodeLock();
  useGlobalKeys();
  useAppActions();
  useWindowClose();
  useConnectionWindow();
  useBackupScheduler();
  const compact = useWindowMode();
  const gridPrefs = useGridPrefsFromSettings();

  return (
    <GridPrefsContext.Provider value={gridPrefs}>
      <div className={`app${compact ? " compact" : ""}`}>
        {!compact && <Toolbar />}
        {!compact && <ConnectionTabs />}
        <div className="main">
          <div className="home-wrap" style={{ display: view === HOME ? "flex" : "none" }}>
            <Home />
          </div>
          {Object.values(workspaces).map((ws) => (
            <WorkspaceView key={ws.connectionId} ws={ws} visible={view === ws.connectionId} />
          ))}
        </div>
        {consoleOpen && <ConsolePanel />}
        <Overlays />
        <LockScreen />
      </div>
    </GridPrefsContext.Provider>
  );
}
