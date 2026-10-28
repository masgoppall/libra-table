// "Backup Database" / "Restore Database" on the welcome window: pick a connection, then
// it opens and the backup or restore dialog shows on top of it.

import { useState } from "react";
import { DriverCircle } from "@/components/DriverLogo";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { EmptyState } from "@/components/StateView";
import { builtInDialect, connectionSubtitle } from "@/domain/connection";
import { openConnection } from "@/services/connect";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type ConnectionView } from "@/types";

export type ToolKind = "backup" | "restore";

const TITLE: Record<ToolKind, string> = {
  backup: "Backup Database",
  restore: "Restore Database",
};

/** Backups need a built-in driver; restoring also needs a server (not SQLite). */
export function supportsTool(c: ConnectionView, kind: ToolKind): boolean {
  const dialect = builtInDialect(c.driver);
  return !!dialect && (kind === "backup" || dialect !== "sqlite");
}

async function openTool(c: ConnectionView, kind: ToolKind) {
  try {
    if (!(await openConnection(c))) return;
    const ws = store.workspace(c.id);
    if (ws) store.get().setServerDialog({ kind, connectionId: c.id, database: ws.database });
  } catch (e) {
    showError(e);
  }
}

export function ToolPicker(props: {
  kind: ToolKind;
  connections: ConnectionView[];
  onClose: () => void;
}) {
  const choices = props.connections.filter((c) => supportsTool(c, props.kind));
  const [selected, setSelected] = useState<string | null>(choices[0]?.id ?? null);
  const pick = (id: string | null) => {
    const c = choices.find((x) => x.id === id);
    if (!c) return;
    props.onClose();
    void openTool(c, props.kind);
  };
  return (
    <Modal
      size="sm"
      title={TITLE[props.kind]}
      subtitle="Choose the connection"
      onClose={props.onClose}
      footer={
        <ModalActions
          onCancel={props.onClose}
          onOk={() => pick(selected)}
          okLabel="Continue"
          disabled={!selected}
        />
      }
    >
      {choices.length === 0 ? (
        <EmptyState compact title="No connection supports this yet" />
      ) : (
        <div className="tool-picker">
          {choices.map((c) => (
            <button
              key={c.id}
              className={`tool-picker-row${selected === c.id ? " selected" : ""}`}
              onClick={() => setSelected(c.id)}
              onDoubleClick={() => pick(c.id)}
            >
              <DriverCircle driver={c.driver} size={24} />
              <span className="tool-picker-name">{c.name}</span>
              <span className="tool-picker-sub">{connectionSubtitle(c)}</span>
            </button>
          ))}
        </div>
      )}
    </Modal>
  );
}
