// Shows the newest dialog from the store's queue.

import { type DialogResult, useStore } from "@/store";
import { ConfirmDialog } from "./ConfirmDialog";
import { HostKeyDialog } from "./HostKeyDialog";
import { PromptDialog } from "./PromptDialog";

export function DialogHost() {
  const top = useStore((s) => s.dialogs[s.dialogs.length - 1]);
  const closeDialog = useStore((s) => s.closeDialog);
  if (!top) return null;
  const done = (r: DialogResult) => closeDialog(top.id, r);
  switch (top.spec.kind) {
    case "prompt":
      return <PromptDialog key={top.id} spec={top.spec} done={done} />;
    case "confirm":
      return <ConfirmDialog key={top.id} spec={top.spec} done={done} />;
    case "hostKey":
      return <HostKeyDialog key={top.id} spec={top.spec} done={done} />;
  }
}
