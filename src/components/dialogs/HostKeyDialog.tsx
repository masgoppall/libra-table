// SSH-4: show the server fingerprint; "Continue & update" or "Cancel".

import { AlertTriangle, ShieldAlert } from "lucide-react";
import { type DialogResult, type DialogSpec } from "@/store";
import { Modal } from "../Modal";
import { ModalActions } from "../ModalActions";

type HostKeySpec = Extract<DialogSpec, { kind: "hostKey" }>;

export function HostKeyDialog({
  spec,
  done,
}: {
  spec: HostKeySpec;
  done: (r: DialogResult) => void;
}) {
  const e = spec.error;
  const changed = e.kind === "ssh_host_key_changed";
  const cancel = () => done({ ok: false });
  const icon = changed ? (
    <ShieldAlert size={18} color="var(--danger)" />
  ) : (
    <AlertTriangle size={18} color="var(--warn)" />
  );
  return (
    <Modal
      title={changed ? "SSH host key changed" : "Verify SSH host"}
      icon={icon}
      onClose={cancel}
      footer={
        <ModalActions
          onCancel={cancel}
          onOk={() => done({ ok: true })}
          okLabel={changed ? "Continue & update known hosts" : "Trust & continue"}
          danger={changed}
          focusCancel
        />
      }
    >
      <div className={`callout ${changed ? "error" : "warn"}`}>
        <div>
          <div>{e.message}</div>
          {e.hint && <div className="hint">{e.hint}</div>}
        </div>
      </div>
      <div className="form-grid">
        <label>Host</label>
        <span className="mono selectable">
          {e.data?.host}:{e.data?.port}
        </span>
        <label>Key type</label>
        <span className="mono">{e.data?.algorithm}</span>
        <label>Fingerprint</label>
        <span className="mono selectable break-all">{e.data?.fingerprint}</span>
      </div>
    </Modal>
  );
}
