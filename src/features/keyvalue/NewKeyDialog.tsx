// "New key": name, type, optional TTL and the first value (a field, an item, a
// member with its score, or a stream entry), created with one command.

import { useState } from "react";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type KeyTarget, runEdit } from "./edits";
import { KEY_TYPES, type KeyType } from "./types";

/** What each type asks for besides the name. */
const INPUTS: Record<KeyType, string[]> = {
  string: ["Value"],
  hash: ["Field", "Value"],
  list: ["Value"],
  set: ["Member"],
  zset: ["Member", "Score"],
  stream: ["Field", "Value"],
};

/** The `value` of the create edit for the typed inputs. */
export function createValue(type: KeyType, [a = "", b = ""]: string[]): unknown {
  switch (type) {
    case "string":
      return a;
    case "hash":
    case "stream":
      return [{ field: a, value: b }];
    case "list":
    case "set":
      return [a];
    case "zset":
      return [{ member: a, score: b || "0" }];
  }
}

export function NewKeyDialog(p: {
  target: KeyTarget;
  onClose: () => void;
  onCreated: (key: string) => void;
}) {
  const [name, setName] = useState("");
  const [type, setType] = useState<KeyType>("string");
  const [ttl, setTtl] = useState("");
  const [inputs, setInputs] = useState<string[]>(["", ""]);
  const [busy, setBusy] = useState(false);
  const ttlValue = ttl.trim() === "" ? null : Number(ttl);
  const valid = name !== "" && (ttlValue === null || (Number.isInteger(ttlValue) && ttlValue > 0));

  const create = async () => {
    setBusy(true);
    const edit = {
      op: "create" as const,
      key: name,
      type,
      value: createValue(type, inputs),
      ttl: ttlValue,
    };
    const ok = await runEdit(p.target, edit, `Created ${name}.`);
    setBusy(false);
    if (ok) p.onCreated(name);
  };

  return (
    <Modal
      title="New key"
      size="sm"
      onClose={p.onClose}
      footer={
        <ModalActions
          onCancel={p.onClose}
          onOk={() => void create()}
          okLabel="Create"
          disabled={!valid}
          busy={busy}
        />
      }
    >
      <div className="form-grid">
        <label htmlFor="kv-new-name">Key</label>
        <input
          id="kv-new-name"
          className="input"
          autoFocus
          value={name}
          placeholder="user:42"
          onChange={(e) => setName(e.target.value)}
        />
        <label htmlFor="kv-new-type">Type</label>
        <select
          id="kv-new-type"
          className="select"
          value={type}
          onChange={(e) => setType(e.target.value as KeyType)}
        >
          {KEY_TYPES.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        {INPUTS[type].map((label, i) => (
          <InputRow
            key={`${type}-${label}`}
            label={label}
            value={inputs[i] ?? ""}
            multiline={type === "string"}
            onChange={(v) => setInputs(inputs.map((x, j) => (j === i ? v : x)))}
          />
        ))}
        <label htmlFor="kv-new-ttl">TTL (seconds)</label>
        <input
          id="kv-new-ttl"
          className="input"
          placeholder="No expiry"
          value={ttl}
          onChange={(e) => setTtl(e.target.value)}
        />
      </div>
    </Modal>
  );
}

function InputRow(p: {
  label: string;
  value: string;
  multiline: boolean;
  onChange: (v: string) => void;
}) {
  const id = `kv-new-${p.label.toLowerCase()}`;
  return (
    <>
      <label htmlFor={id}>{p.label}</label>
      {p.multiline ? (
        <textarea
          id={id}
          className="input kv-text-sm mono"
          value={p.value}
          onChange={(e) => p.onChange(e.target.value)}
        />
      ) : (
        <input
          id={id}
          className="input"
          value={p.value}
          onChange={(e) => p.onChange(e.target.value)}
        />
      )}
    </>
  );
}
