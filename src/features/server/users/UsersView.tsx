// The "Users" tab: Postgres roles or MySQL accounts, their privileges, and every change
// previewed as SQL before it runs.

import { RefreshCw, Search, UserPlus, Users as UsersIcon } from "lucide-react";
import { useState } from "react";
import { PaddedError } from "@/components/ErrorCallout";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions, withShortcut } from "@/lib/actions";
import { tabDb } from "@/store";
import { type Grantee, type PgRole, type Workspace, type WorkspaceTab } from "@/types";
import { GrantDialog, type GrantMode } from "./GrantDialog";
import { GrantsPanel } from "./GrantsPanel";
import { AccountDialog } from "./AccountDialog";
import { RoleDialog } from "./RoleDialog";
import { dropGrantee, setPassword } from "./userFlows";
import { AccountTable, RoleTable, type RowActions } from "./UserTables";
import { type Users, useUsers } from "./useUsers";

type OpenDialog =
  | { kind: "role"; role?: PgRole }
  | { kind: "account" }
  | { kind: "grants"; mode: GrantMode; grantee: Grantee };

function UsersToolbar(props: { u: Users; pg: boolean; onNew: () => void }) {
  const { u } = props;
  return (
    <div className="toolbar">
      <span className="toolbar-title">
        <UsersIcon size={15} />
        <strong>{props.pg ? "Roles" : "Users"}</strong>
      </span>
      <div className="search-field">
        <Search size={13} className="faint" />
        <input
          className="input sm"
          placeholder="Search"
          value={u.text}
          onChange={(e) => u.setText(e.target.value)}
        />
      </div>
      <span className="spacer" />
      <button className="btn sm" onClick={props.onNew}>
        <UserPlus size={13} /> {props.pg ? "New role…" : "New user…"}
      </button>
      <button className="btn sm" onClick={u.reload} title={withShortcut("Refresh", "refresh")}>
        {u.loading ? <Spinner /> : <RefreshCw size={13} />} Refresh
      </button>
    </div>
  );
}

function Dialogs(props: {
  ws: Workspace;
  tab: WorkspaceTab;
  u: Users;
  open: OpenDialog | null;
  close: () => void;
}) {
  const { ws, u, open } = props;
  const db = tabDb(ws, props.tab);
  const done = (changed: boolean) => {
    props.close();
    if (changed) u.reload();
  };
  if (!open) return null;
  if (open.kind === "account") return <AccountDialog ws={ws} db={db} onClose={done} />;
  if (open.kind === "role") {
    const names = u.list.roles.map((r) => r.name);
    return <RoleDialog ws={ws} db={db} role={open.role} allRoles={names} onClose={done} />;
  }
  return (
    <GrantDialog
      ws={ws}
      db={db}
      grantee={open.grantee}
      mode={open.mode}
      options={u.list.privilegeOptions}
      onClose={done}
    />
  );
}

export function UsersView(props: { ws: Workspace; tab: WorkspaceTab; visible: boolean }) {
  const { ws, tab, visible } = props;
  const db = tabDb(ws, tab);
  const u = useUsers(ws.connectionId, db, visible);
  const [open, setOpen] = useState<OpenDialog | null>(null);
  useActions(LAYER.tab, visible, { refresh: u.reload });
  const pg = ws.info.dialect === "postgres";
  const after = (done: Promise<boolean>) => void done.then((ok) => ok && u.reload());
  const actions: RowActions = {
    onEdit: (role) => setOpen({ kind: "role", role }),
    onPassword: (g) => after(setPassword(ws, db, g)),
    onDrop: (g) => {
      after(dropGrantee(ws, db, g));
      u.setSelected(null);
    },
  };
  const table = { text: u.text, selected: u.selected, onSelect: u.setSelected, actions };
  const grantee = u.selected;
  return (
    <div className="tab-body">
      <UsersToolbar u={u} pg={pg} onNew={() => setOpen({ kind: pg ? "role" : "account" })} />
      <PaddedError error={u.error} />
      <div className="server-scroll">
        {pg ? (
          <RoleTable roles={u.list.roles} {...table} />
        ) : (
          <AccountTable accounts={u.list.accounts} {...table} />
        )}
      </div>
      {grantee && (
        <GrantsPanel
          grantee={grantee}
          grants={u.grants}
          database={pg ? (tab.database ?? ws.database) : undefined}
          onGrant={() => setOpen({ kind: "grants", mode: "grant", grantee })}
          onRevoke={() => setOpen({ kind: "grants", mode: "revoke", grantee })}
        />
      )}
      <Dialogs ws={ws} tab={tab} u={u} open={open} close={() => setOpen(null)} />
    </div>
  );
}
