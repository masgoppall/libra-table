// What the selected role or account was granted, with Grant… / Revoke….

import { ShieldCheck, ShieldMinus, ShieldPlus } from "lucide-react";
import { type Grantee } from "@/types";
import { granteeLabel } from "./userModel";

export function GrantsPanel(props: {
  grantee: Grantee;
  grants: string[] | null;
  /** Postgres: the database the schema and table grants are listed for. */
  database?: string;
  onGrant: () => void;
  onRevoke: () => void;
}) {
  const where = props.database ? ` in ${props.database}` : "";
  return (
    <div className="grants-panel">
      <div className="toolbar">
        <span className="toolbar-title">
          <ShieldCheck size={15} />
          <strong>{granteeLabel(props.grantee)}</strong>
          <span className="muted">privileges{where}</span>
        </span>
        <span className="spacer" />
        <button className="btn sm" onClick={props.onGrant}>
          <ShieldPlus size={13} /> Grant…
        </button>
        <button className="btn sm" onClick={props.onRevoke}>
          <ShieldMinus size={13} /> Revoke…
        </button>
      </div>
      <div className="grants-list">
        {props.grants?.map((g) => (
          <div key={g} className="grant-line mono selectable">
            {g}
          </div>
        ))}
        {props.grants?.length === 0 && (
          <div className="message faint">No privileges granted explicitly.</div>
        )}
      </div>
    </div>
  );
}
