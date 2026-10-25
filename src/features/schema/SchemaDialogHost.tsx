import { useStore } from "@/store";
import { CreateDatabaseDialog } from "./CreateDatabaseDialog";

/** The "New database" dialog, opened from anywhere through the store. */
export function SchemaDialogHost() {
  const ws = useStore((s) => (s.createDatabaseFor ? s.workspaces[s.createDatabaseFor] : undefined));
  const close = useStore((s) => s.setCreateDatabaseFor);
  if (!ws) return null;
  return <CreateDatabaseDialog ws={ws} onClose={() => close(null)} />;
}
