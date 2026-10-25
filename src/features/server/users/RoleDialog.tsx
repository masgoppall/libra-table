// Create or edit a Postgres role: attributes, limits, memberships and a password.

import { useState } from "react";
import { Check } from "@/components/Check";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type Db, type PgRole, type RoleSpec, type UserChange, type Workspace } from "@/types";
import { runUserChange } from "./userFlows";
import { emptyRole, parseConnLimit, roleToSpec } from "./userModel";

type Flag = "login" | "superuser" | "createDb" | "createRole" | "replication";

const FLAGS: [Flag, string][] = [
  ["login", "Can log in"],
  ["superuser", "Superuser"],
  ["createDb", "Create databases"],
  ["createRole", "Create roles"],
  ["replication", "Replication"],
];

function MemberOf(props: { roles: string[]; value: string[]; onChange: (v: string[]) => void }) {
  if (props.roles.length === 0) return <span className="faint">No other roles.</span>;
  const toggle = (r: string, on: boolean) =>
    props.onChange(on ? [...props.value, r] : props.value.filter((x) => x !== r));
  return (
    <div className="check-list">
      {props.roles.map((r) => (
        <Check key={r} checked={props.value.includes(r)} onChange={(on) => toggle(r, on)}>
          <span className="mono">{r}</span>
        </Check>
      ))}
    </div>
  );
}

function changeOf(role: PgRole | undefined, spec: RoleSpec, password: string): UserChange {
  const pw = password ? password : null;
  return role
    ? { kind: "alter_role", name: role.name, role: spec, password: pw }
    : { kind: "create_role", role: spec, password: pw };
}

export function RoleDialog(props: {
  ws: Workspace;
  db: Db;
  /** The role to edit; none to create one. */
  role?: PgRole;
  allRoles: string[];
  onClose: (changed: boolean) => void;
}) {
  const { role } = props;
  const [spec, setSpec] = useState<RoleSpec>(role ? roleToSpec(role) : emptyRole());
  const [limit, setLimit] = useState(spec.connLimit === null ? "" : String(spec.connLimit));
  const [password, setPassword] = useState("");
  const set = (patch: Partial<RoleSpec>) => setSpec((s) => ({ ...s, ...patch }));
  const setFlag = (flag: Flag, on: boolean) =>
    setSpec((s) => {
      const next = { ...s };
      next[flag] = on;
      return next;
    });
  const connLimit = parseConnLimit(limit);
  const valid = spec.name.trim() !== "" && connLimit !== undefined;

  const submit = async () => {
    if (!valid) return;
    const full: RoleSpec = { ...spec, name: spec.name.trim(), connLimit: connLimit ?? null };
    const ok = await runUserChange(props.ws, props.db, changeOf(role, full, password), {
      title: role ? `Change role ${role.name}` : "Create role",
      okLabel: role ? "Apply" : "Create role",
      success: role ? `Role ${full.name} changed.` : `Role ${full.name} created.`,
    });
    if (ok) props.onClose(true);
  };

  const others = props.allRoles.filter((r) => r !== role?.name);
  const footer = (
    <ModalActions
      onCancel={() => props.onClose(false)}
      onOk={() => void submit()}
      okLabel={role ? "Review changes…" : "Create…"}
      disabled={!valid}
    />
  );
  return (
    <Modal
      title={role ? `Role ${role.name}` : "New role"}
      onClose={() => props.onClose(false)}
      footer={footer}
    >
      <div className="form-grid">
        <label>Name</label>
        <input
          className="input mono"
          autoFocus
          placeholder="role_name"
          value={spec.name}
          onChange={(e) => set({ name: e.target.value })}
        />
        <label>Attributes</label>
        <div className="check-list">
          {FLAGS.map(([key, label]) => (
            <Check key={key} checked={spec[key]} onChange={(on) => setFlag(key, on)}>
              {label}
            </Check>
          ))}
        </div>
        <label>Connection limit</label>
        <input
          className={`input num-input${connLimit === undefined ? " invalid" : ""}`}
          placeholder="no limit"
          value={limit}
          onChange={(e) => setLimit(e.target.value)}
        />
        <label>Valid until</label>
        <input
          className="input"
          placeholder="forever (or e.g. 2027-01-31)"
          value={spec.validUntil ?? ""}
          onChange={(e) => set({ validUntil: e.target.value || null })}
        />
        <label>Member of</label>
        <MemberOf roles={others} value={spec.memberOf} onChange={(memberOf) => set({ memberOf })} />
        <label>{role ? "New password" : "Password"}</label>
        <input
          className="input"
          type="password"
          autoComplete="new-password"
          placeholder={role ? "unchanged" : "none"}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </div>
      <div className="faint small">
        The password is sent once to the server and isn&apos;t stored; the preview shows it masked.
      </div>
    </Modal>
  );
}
