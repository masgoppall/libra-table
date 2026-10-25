import { ShieldAlert } from "lucide-react";
import { useState } from "react";
import { type DialogResult, type DialogSpec } from "@/store";
import { Check } from "../Check";
import { Modal } from "../Modal";
import { ModalActions } from "../ModalActions";

type ConfirmSpec = Extract<DialogSpec, { kind: "confirm" }>;

function StatementList({ statements }: { statements: string[] }) {
  return (
    <div className="statement-list">
      {statements.map((sql, i) => (
        <pre key={i} className="mono">
          {sql}
        </pre>
      ))}
    </div>
  );
}

function TypeToConfirm(props: {
  word: string;
  value: string;
  onChange: (v: string) => void;
  onEnter: () => void;
}) {
  return (
    <label className="field">
      <span className="field-label">
        Type <strong className="mono">{props.word}</strong> to confirm
      </span>
      <input
        className="input mono"
        autoFocus
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && props.onEnter()}
      />
    </label>
  );
}

export function ConfirmDialog({
  spec,
  done,
}: {
  spec: ConfirmSpec;
  done: (r: DialogResult) => void;
}) {
  const [typed, setTyped] = useState("");
  const [checked, setChecked] = useState(false);
  const blocked = !!spec.typeToConfirm && typed !== spec.typeToConfirm;
  const ok = () => !blocked && done({ ok: true, checked });
  const cancel = () => done({ ok: false });
  const hasDetails = !!spec.details?.length;
  return (
    <Modal
      title={spec.title}
      size={hasDetails ? "lg" : "sm"}
      icon={spec.danger ? <ShieldAlert size={18} color="var(--danger)" /> : undefined}
      onClose={cancel}
      footer={
        <ModalActions
          onCancel={cancel}
          onOk={ok}
          okLabel={spec.okLabel ?? "OK"}
          danger={spec.danger}
          disabled={blocked}
          focusCancel={!spec.typeToConfirm}
        />
      }
    >
      <div>{spec.message}</div>
      {hasDetails && <StatementList statements={spec.details ?? []} />}
      {spec.checkbox && (
        <Check checked={checked} onChange={setChecked}>
          {spec.checkbox}
        </Check>
      )}
      {spec.typeToConfirm && (
        <TypeToConfirm word={spec.typeToConfirm} value={typed} onChange={setTyped} onEnter={ok} />
      )}
    </Modal>
  );
}
