// The app shell: title bar, the connections page or a workspace, and global overlays.

import { useEffect } from "react";
import { DialogHost } from "@/components/dialogs/DialogHost";
import { Toasts } from "@/components/Toasts";
import { Home } from "@/features/connections/Home";
import { LockScreen } from "@/features/lock/LockScreen";
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
import { TopBar } from "./TopBar";
import { useAppActions } from "./useAppActions";
import { useAppearance } from "./useAppearance";
import { useCustomShortcuts, useGlobalKeys } from "./useGlobalKeys";

function Overlays() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const shortcutsOpen = useStore((s) => s.shortcutsOpen);
  const { setSettingsOpen, setShortcutsOpen } = useStore.getState();
  return (
    <>
      <Palette />
      <SchemaDialogHost />
      <ServerDialogHost />
      {settingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} />}
      {shortcutsOpen && <ShortcutsDialog onClose={() => setShortcutsOpen(false)} />}
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
    const { loadConnections, loadSettings } = useStore.getState();
    loadConnections().catch(showError);
    loadSettings().catch(showError);
  }, []);
  useAppearance();
  useCustomShortcuts();
  usePasscodeLock();
  useGlobalKeys();
  useAppActions();

  return (
    <div className="app">
      <TopBar />
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
  );
}
