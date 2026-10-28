// The welcome window's left side: the app's mark, name and version, the connections
// open in the last session (STO-5), and backup / restore without opening a workspace.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { type ConnectionView } from "@/types";
import { TrayArrowIcon } from "./icons";

function useVersion(): string {
  const [version, setVersion] = useState("");
  useEffect(() => {
    api
      .appInfo()
      .then((info) => setVersion(info.version))
      .catch(() => undefined);
  }, []);
  return version;
}

/** Where TablePlus shows its trial notice: the connections to reopen, if any. */
function LastSession(props: { connections: ConnectionView[]; onReopen: () => void }) {
  const n = props.connections.length;
  if (n === 0) return null;
  const what = n === 1 ? (props.connections[0]?.name ?? "") : `${n} connections`;
  return (
    <div className="home-session">
      <div className="home-session-note" title={props.connections.map((c) => c.name).join(", ")}>
        Last session: {what}
      </div>
      <div className="home-session-actions">
        <button className="home-small-btn" onClick={props.onReopen}>
          Reopen
        </button>
      </div>
    </div>
  );
}

export function WelcomeSide(props: {
  restorable: ConnectionView[];
  onReopen: () => void;
  onBackup: () => void;
  onRestore: () => void;
}) {
  const version = useVersion();
  return (
    <aside className="home-side" data-tauri-drag-region>
      <img className="home-mark" src="/app-icon.png" alt="" draggable={false} />
      <h1 className="home-name">LibraTable</h1>
      <div className="home-version">{version && `Version ${version}`}</div>
      <LastSession connections={props.restorable} onReopen={props.onReopen} />
      <div className="home-tools">
        <button className="home-tool" onClick={props.onBackup}>
          <TrayArrowIcon direction="up" size={20} />
          Backup Database
        </button>
        <button className="home-tool" onClick={props.onRestore}>
          <TrayArrowIcon direction="down" size={20} />
          Restore Database
        </button>
      </div>
    </aside>
  );
}
