// Asks for the values of the `:name` variables of a run; the last values are kept.

import { Variable } from "lucide-react";
import { useState } from "react";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type QueryVariable, type VariableMode } from "@/types";

const MODES: [VariableMode, string][] = [
  ["auto", "Auto"],
  ["text", "Text"],
  ["number", "Number"],
  ["null", "NULL"],
  ["raw", "Raw SQL"],
];

export function VariablesDialog(props: {
  initial: QueryVariable[];
  onDone: (values: QueryVariable[] | null) => void;
}) {
  const [values, setValues] = useState(props.initial);
  const set = (i: number, patch: Partial<QueryVariable>) =>
    setValues((vs) => vs.map((v, j) => (j === i ? { ...v, ...patch } : v)));
  const ok = () => props.onDone(values);
  const cancel = () => props.onDone(null);
  return (
    <Modal
      title="Query variables"
      icon={<Variable size={16} />}
      subtitle="Values go into the SQL as quoted literals (Auto: numbers stay numbers)."
      onClose={cancel}
      footer={<ModalActions onCancel={cancel} onOk={ok} okLabel="Run" />}
    >
      <form
        className="form-grid variables-form"
        onSubmit={(e) => {
          e.preventDefault();
          ok();
        }}
      >
        {values.map((v, i) => (
          <VariableRow key={v.name} v={v} first={i === 0} onChange={(p) => set(i, p)} />
        ))}
        <button type="submit" hidden />
      </form>
    </Modal>
  );
}

function VariableRow(props: {
  v: QueryVariable;
  first: boolean;
  onChange: (p: Partial<QueryVariable>) => void;
}) {
  const { v } = props;
  const id = `var-${v.name}`;
  return (
    <>
      <label htmlFor={id} className="mono">
        :{v.name}
      </label>
      <div className="row tight">
        <input
          id={id}
          className="input sm grow mono"
          value={v.value}
          disabled={v.mode === "null"}
          autoFocus={props.first}
          placeholder={v.mode === "raw" ? "SQL, inserted as typed" : "value"}
          onChange={(e) => props.onChange({ value: e.target.value })}
        />
        <select
          className="select sm"
          aria-label={`How :${v.name} is inserted`}
          value={v.mode}
          onChange={(e) => props.onChange({ mode: e.target.value as VariableMode })}
        >
          {MODES.map(([m, label]) => (
            <option key={m} value={m}>
              {label}
            </option>
          ))}
        </select>
      </div>
    </>
  );
}
