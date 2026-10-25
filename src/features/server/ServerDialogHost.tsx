// Backup, restore and "Open in command line", opened from anywhere through the store.

import { useStore } from "@/store";
import { BackupDialog } from "./backup/BackupDialog";
import { RestoreDialog } from "./backup/RestoreDialog";
import { CommandLineDialog } from "./cli/CommandLineDialog";

export function ServerDialogHost() {
  const open = useStore((s) => s.serverDialog);
  const ws = useStore((s) => (open ? s.workspaces[open.connectionId] : undefined));
  const close = () => useStore.getState().setServerDialog(null);
  if (!open || !ws) return null;
  const p = { ws, database: open.database, onClose: close };
  switch (open.kind) {
    case "backup":
      return <BackupDialog {...p} />;
    case "restore":
      return <RestoreDialog {...p} />;
    case "command-line":
      return <CommandLineDialog {...p} />;
  }
}
