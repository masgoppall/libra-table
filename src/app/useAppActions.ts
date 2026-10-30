// Actions that work everywhere: settings, text size, switching connections.

import { LAYER, tabNumberHandlers, useActions } from "@/lib/actions";
import { clamp } from "@/lib/drag";
import { showError, toastInfo } from "@/services/errors";
import { HOME, store, useStore } from "@/store";
import { DEFAULT_FONT_SIZE } from "./useAppearance";

const UI_FONT = { min: 11, max: 16 };
const EDITOR_FONT = { min: 10, max: 24 };

/** 0 resets both sizes; ±1 steps them within their limits. */
function bumpFont(step: number) {
  const { settings, saveSettings } = store.get();
  if (!settings) return;
  const sizes =
    step === 0
      ? { uiFontSize: DEFAULT_FONT_SIZE, editorFontSize: DEFAULT_FONT_SIZE }
      : {
          uiFontSize: clamp(settings.uiFontSize + step, UI_FONT.min, UI_FONT.max),
          editorFontSize: clamp(settings.editorFontSize + step, EDITOR_FONT.min, EDITOR_FONT.max),
        };
  saveSettings(sizes).catch(showError);
}

export function useAppActions() {
  const view = useStore((s) => s.view);
  const openIds = useStore((s) => Object.keys(s.workspaces).join("\n"));
  const { setView, setSettingsOpen, setShortcutsOpen, setConsoleOpen, requestNewConnection } =
    store.get();
  const ids = openIds ? openIds.split("\n") : [];
  const views = [HOME, ...ids];

  const step = (dir: 1 | -1) => {
    const at = views.indexOf(view);
    setView(views[(at + dir + views.length) % views.length] ?? HOME);
  };

  useActions(LAYER.app, true, {
    settings: () => setSettingsOpen(true),
    "backup-schedules": () => store.get().setBackupSchedulesOpen(true),
    shortcuts: () => setShortcutsOpen(true),
    lock: () =>
      store.get().passcodeEnabled
        ? store.get().setLocked(true)
        : toastInfo("Set a passcode in Settings → Security first."),
    "toggle-console": () => setConsoleOpen(!store.get().consoleOpen),
    home: () => setView(HOME),
    "new-connection": requestNewConnection,
    "font-bigger": () => bumpFont(1),
    "font-smaller": () => bumpFont(-1),
    "font-reset": () => bumpFont(0),
    "next-tab": () => step(1),
    "prev-tab": () => step(-1),
    // On the home screen ⌘1…⌘9 pick an open connection.
    ...tabNumberHandlers((n) => {
      const id = ids[n - 1];
      if (!id) return false;
      setView(id);
    }),
  });
}
