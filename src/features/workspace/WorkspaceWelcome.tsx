// What an empty workspace shows: the connection and the first things to do.

import { Database, Search, SquareTerminal, TablePropertiesIcon } from "lucide-react";
import { Kbd } from "@/components/Kbd";
import { DRIVER_LABEL } from "@/domain/connection";
import { type ActionId, dispatch, shortcutFor } from "@/lib/actions";
import { useConnection } from "@/store";
import { type Workspace } from "@/types";

function Action(props: { action: ActionId; icon: React.ReactNode; label: string }) {
  return (
    <button className="welcome-action" onClick={() => dispatch(props.action)}>
      {props.icon}
      <span>{props.label}</span>
      <Kbd>{shortcutFor(props.action)}</Kbd>
    </button>
  );
}

export function WorkspaceWelcome({ ws }: { ws: Workspace }) {
  const conn = useConnection(ws.connectionId);
  return (
    <div className="welcome">
      <div className="welcome-card">
        <div className="welcome-title">{conn?.name}</div>
        <div className="welcome-sub">
          {DRIVER_LABEL[ws.info.driver]} {ws.info.serverVersion}
          {ws.database ? ` · ${ws.database}` : ""}
          {ws.info.throughTunnel ? " · via SSH tunnel" : ""}
        </div>
        <div className="welcome-actions">
          <Action action="open-anything" icon={<Search size={16} />} label="Open a table" />
          {ws.caps.query && (
            <Action action="new-query" icon={<SquareTerminal size={16} />} label="New query" />
          )}
          {ws.caps.createTable && (
            <Action action="new-table" icon={<TablePropertiesIcon size={16} />} label="New table" />
          )}
          {ws.caps.switchDatabase && (
            <Action
              action="switch-database"
              icon={<Database size={16} />}
              label="Switch database"
            />
          )}
        </div>
      </div>
    </div>
  );
}
