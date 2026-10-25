import { useState } from "react";
import { type DialogResult, type DialogSpec } from "@/store";
import { Check } from "../Check";
import { Modal } from "../Modal";
import { ModalActions } from "../ModalActions";

type PromptSpec = Extract<DialogSpec, { kind: "prompt" }>;

export function PromptDialog({
  spec,
  done,
}: {
  spec: PromptSpec;
  done: (r: DialogResult) => void;
}) {
  const [value, setValue] = useState(spec.initial ?? "");
  const [checked, setChecked] = useState(!!spec.checked);
  const ok = () => done({ ok: true, value, checked });
  const cancel = () => done({ ok: false });
  return (
    <Modal
      title={spec.title}
      size="sm"
      onClose={cancel}
      footer={
        <ModalActions
          onCancel={cancel}
          onOk={ok}
          okLabel={spec.okLabel ?? "OK"}
          danger={spec.danger}
        />
      }
    >
      {spec.message && <div className="muted">{spec.message}</div>}
      <form
        className="col"
        onSubmit={(e) => {
          e.preventDefault();
          ok();
        }}
      >
        <label className="field">
          <span className="field-label">{spec.label}</span>
          <input
            className="input"
            type={spec.secret ? "password" : "text"}
            value={value}
            placeholder={spec.placeholder}
            autoFocus
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setValue(e.target.value)}
          />
        </label>
        {spec.checkbox && (
          <Check checked={checked} onChange={setChecked}>
            {spec.checkbox}
          </Check>
        )}
      </form>
    </Modal>
  );
}
