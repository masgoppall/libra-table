// Grant or revoke privileges on the server, a database, a schema, all its tables or one
// table. Only privileges the target accepts are offered (Rust checks them again).

import { useState } from "react";
import { Check } from "@/components/Check";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import {
  type Db,
  type Grantee,
  type PrivilegeOption,
  type TargetKind,
  type UserChange,
  type Workspace,
} from "@/types";
import { useGrantTargets } from "./useGrantTargets";
import { runUserChange } from "./userFlows";
import {
  buildTarget,
  granteeLabel,
  privilegesFor,
  TARGET_LABELS,
  type TargetFields,
  togglePrivilege,
} from "./userModel";

export type GrantMode = "grant" | "revoke";

function NameSelect(props: {
  label: string;
  names: string[];
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <>
      <label>{props.label}</label>
      <select
        className="select"
        aria-label={props.label}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      >
        <option value="">Choose…</option>
        {props.names.map((n) => (
          <option key={n} value={n}>
            {n}
          </option>
        ))}
      </select>
    </>
  );
}

function TargetPickers(props: {
  ws: Workspace;
  db: Db;
  kind: TargetKind;
  fields: TargetFields;
  set: (patch: Partial<TargetFields>) => void;
}) {
  const { kind, fields, set } = props;
  const names = useGrantTargets(props.ws, props.db, fields);
  const pg = props.ws.info.dialect === "postgres";
  const container = pg ? "Schema" : "Database";
  return (
    <>
      {kind === "database" && (
        <NameSelect
          label="Database"
          names={names.databases}
          value={fields.database}
          onChange={(database) => set({ database })}
        />
      )}
      {(kind === "schema" || kind === "all_tables" || kind === "table") && (
        <NameSelect
          label={container}
          names={names.schemas}
          value={fields.schema}
          onChange={(schema) => set({ schema, table: "" })}
        />
      )}
      {kind === "table" && (
        <NameSelect
          label="Table"
          names={names.tables}
          value={fields.table}
          onChange={(table) => set({ table })}
        />
      )}
    </>
  );
}

function initialFields(ws: Workspace): TargetFields {
  const pg = ws.info.dialect === "postgres";
  return { database: ws.database, schema: pg ? ws.schema : ws.database, table: "" };
}

export function GrantDialog(props: {
  ws: Workspace;
  db: Db;
  grantee: Grantee;
  mode: GrantMode;
  options: PrivilegeOption[];
  onClose: (changed: boolean) => void;
}) {
  const kinds = props.options.map((o) => o.target);
  const [kind, setKind] = useState<TargetKind>(kinds[0] ?? "database");
  const [fields, setFields] = useState<TargetFields>(initialFields(props.ws));
  const [picked, setPicked] = useState<string[]>([]);
  const [grantOption, setGrantOption] = useState(false);
  const target = buildTarget(kind, fields);
  const valid = target !== null && picked.length > 0;
  const label = granteeLabel(props.grantee);
  const verb = props.mode === "grant" ? "Grant" : "Revoke";

  const submit = async () => {
    if (!target || !valid) return;
    const { grantee } = props;
    const change: UserChange =
      props.mode === "grant"
        ? { kind: "grant", grantee, privileges: picked, target, withGrantOption: grantOption }
        : { kind: "revoke", grantee, privileges: picked, target };
    const ok = await runUserChange(props.ws, props.db, change, {
      title: `${verb} privileges`,
      okLabel: verb,
      danger: props.mode === "revoke",
      success: `${props.mode === "grant" ? "Granted to" : "Revoked from"} ${label}.`,
    });
    if (ok) props.onClose(true);
  };

  const pickKind = (k: TargetKind) => {
    setKind(k);
    setPicked([]);
  };
  const footer = (
    <ModalActions
      onCancel={() => props.onClose(false)}
      onOk={() => void submit()}
      okLabel={`${verb}…`}
      disabled={!valid}
    />
  );
  return (
    <Modal
      title={`${verb} privileges · ${label}`}
      onClose={() => props.onClose(false)}
      footer={footer}
    >
      <div className="form-grid">
        <label>On</label>
        <select
          className="select"
          aria-label="On"
          value={kind}
          onChange={(e) => pickKind(e.target.value as TargetKind)}
        >
          {kinds.map((k) => (
            <option key={k} value={k}>
              {TARGET_LABELS[k]}
            </option>
          ))}
        </select>
        <TargetPickers
          ws={props.ws}
          db={props.db}
          kind={kind}
          fields={fields}
          set={(patch) => setFields((f) => ({ ...f, ...patch }))}
        />
        <label>Privileges</label>
        <div className="check-list">
          {privilegesFor(props.options, kind).map((p) => (
            <Check
              key={p}
              checked={picked.includes(p)}
              onChange={(on) => setPicked((xs) => togglePrivilege(xs, p, on))}
            >
              {p}
            </Check>
          ))}
        </div>
        {props.mode === "grant" && (
          <>
            <label>Options</label>
            <Check checked={grantOption} onChange={setGrantOption}>
              With grant option (may grant these to others)
            </Check>
          </>
        )}
      </div>
    </Modal>
  );
}
