// "Open in command line": the psql / mysql / sqlite3 command for the connection, without
// the password (the client asks for it), with Copy and Open in Terminal.

import { AlertTriangle, Copy, SquareTerminal } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "@/api";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { IS_MAC } from "@/lib/actions";
import { copyText } from "@/lib/clipboard";
import { showError, toastSuccess } from "@/services/errors";
import { tabDb } from "@/store";
import { type AppError, type CommandLine, type Workspace } from "@/types";

const TUNNEL_NOTE =
  " The command uses LibraTable's SSH tunnel, so it works while this connection is open.";

export function CommandLineDialog(props: { ws: Workspace; database: string; onClose: () => void }) {
  const { ws } = props;
  const db = tabDb(ws, { database: props.database });
  const [line, setLine] = useState<CommandLine | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    api.commandLine(ws.connectionId, db).then(setLine, (e: AppError) => setError(e));
  }, [ws.connectionId, db]);

  const copy = async () => {
    if (!line) return;
    await copyText(line.command);
    toastSuccess("Command copied.");
  };
  const terminal = async () => {
    try {
      await api.openInTerminal(ws.connectionId, db);
      props.onClose();
    } catch (e) {
      showError(e);
    }
  };

  const footer = (
    <>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Close
      </button>
      <button className="btn" onClick={() => void copy()} disabled={!line}>
        <Copy size={13} /> Copy
      </button>
      {IS_MAC && (
        <button className="btn primary" onClick={() => void terminal()} disabled={!line}>
          <SquareTerminal size={13} /> Open in Terminal
        </button>
      )}
    </>
  );
  return (
    <Modal
      title="Open in command line"
      icon={<SquareTerminal size={18} />}
      onClose={props.onClose}
      footer={footer}
    >
      {!line && !error && (
        <div className="row muted">
          <Spinner /> Finding {ws.info.dialect === "sqlite" ? "sqlite3" : "the client"}…
        </div>
      )}
      {line && (
        <>
          <pre className="command-line mono selectable">{line.command}</pre>
          <div className="faint small">
            The password isn&apos;t part of the command: the client asks for it.
            {line.throughTunnel && TUNNEL_NOTE}
          </div>
          {line.tool.warning && (
            <div className="callout warn">
              <AlertTriangle size={15} className="callout-icon" />
              <div>{line.tool.warning}</div>
            </div>
          )}
        </>
      )}
      <ErrorCallout error={error} />
    </Modal>
  );
}
