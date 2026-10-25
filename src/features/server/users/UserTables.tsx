// The list of Postgres roles or MySQL accounts; a click selects one.

import { KeyRound, Pencil, Trash2 } from "lucide-react";
import { type ReactNode } from "react";
import { type Grantee, type MysqlAccount, type PgRole } from "@/types";
import { accountGrantee, matchesSearch, roleFlags, roleGrantee } from "./userModel";

export interface RowActions {
  onEdit?: (r: PgRole) => void;
  onPassword: (g: Grantee) => void;
  onDrop: (g: Grantee) => void;
}

function Actions(props: { g: Grantee; a: RowActions; edit?: () => void }) {
  const stop = (fn: () => void) => (e: React.MouseEvent) => {
    e.stopPropagation();
    fn();
  };
  return (
    <td className="process-actions">
      {props.edit && (
        <button className="btn xs" onClick={stop(props.edit)}>
          <Pencil size={12} /> Edit…
        </button>
      )}
      <button className="btn xs" onClick={stop(() => props.a.onPassword(props.g))}>
        <KeyRound size={12} /> Password…
      </button>
      <button
        className="btn xs danger-outline"
        title="Drop"
        onClick={stop(() => props.a.onDrop(props.g))}
      >
        <Trash2 size={12} />
      </button>
    </td>
  );
}

function Row(props: { selected: boolean; onSelect: () => void; children: ReactNode }) {
  return (
    <tr className={props.selected ? "selected" : undefined} onClick={props.onSelect}>
      {props.children}
    </tr>
  );
}

export function RoleTable(props: {
  roles: PgRole[];
  text: string;
  selected: Grantee | null;
  onSelect: (g: Grantee) => void;
  actions: RowActions;
}) {
  const rows = props.roles.filter((r) => matchesSearch(props.text, r.name, ...r.memberOf));
  return (
    <table className="edit-table server-table">
      <thead>
        <tr>
          <th>Role</th>
          <th>Login</th>
          <th>Attributes</th>
          <th>Conn. limit</th>
          <th>Valid until</th>
          <th>Member of</th>
          <th className="col-action" />
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <Row
            key={r.name}
            selected={props.selected?.name === r.name}
            onSelect={() => props.onSelect(roleGrantee(r.name))}
          >
            <td className="mono">{r.name}</td>
            <td>{r.login ? "Yes" : "No"}</td>
            <td>{roleFlags(r).join(", ") || <span className="faint">—</span>}</td>
            <td className="num">{r.connLimit < 0 ? "—" : r.connLimit}</td>
            <td>{r.validUntil ?? <span className="faint">—</span>}</td>
            <td>{r.memberOf.join(", ")}</td>
            <Actions
              g={roleGrantee(r.name)}
              a={props.actions}
              edit={() => props.actions.onEdit?.(r)}
            />
          </Row>
        ))}
      </tbody>
    </table>
  );
}

export function AccountTable(props: {
  accounts: MysqlAccount[];
  text: string;
  selected: Grantee | null;
  onSelect: (g: Grantee) => void;
  actions: RowActions;
}) {
  const rows = props.accounts.filter((a) => matchesSearch(props.text, a.user, a.host));
  const isSelected = (a: MysqlAccount) =>
    props.selected?.name === a.user && props.selected.host === a.host;
  return (
    <table className="edit-table server-table">
      <thead>
        <tr>
          <th>User</th>
          <th>Host</th>
          <th>Global privileges</th>
          <th className="col-action" />
        </tr>
      </thead>
      <tbody>
        {rows.map((a) => (
          <Row
            key={`${a.user}@${a.host}`}
            selected={isSelected(a)}
            onSelect={() => props.onSelect(accountGrantee(a))}
          >
            <td className="mono">{a.user}</td>
            <td className="mono">{a.host}</td>
            <td className="process-query">
              {a.privileges.join(", ") || <span className="faint">none</span>}
            </td>
            <Actions g={accountGrantee(a)} a={props.actions} />
          </Row>
        ))}
      </tbody>
    </table>
  );
}
