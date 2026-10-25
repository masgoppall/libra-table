// Create a MySQL account: user, host and an optional password.

import { useState } from "react";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type Db, type Workspace } from "@/types";
import { runUserChange } from "./userFlows";

export function AccountDialog(props: {
  ws: Workspace;
  db: Db;
  onClose: (changed: boolean) => void;
}) {
  const [user, setUser] = useState("");
  const [host, setHost] = useState("%");
  const [password, setPassword] = useState("");
  const valid = user.trim() !== "";

  const submit = async () => {
    if (!valid) return;
    const name = user.trim();
    const where = host.trim() || "%";
    const change = {
      kind: "create_user" as const,
      user: name,
      host: where,
      password: password ? password : null,
    };
    const ok = await runUserChange(props.ws, props.db, change, {
      title: "Create user",
      okLabel: "Create user",
      success: `User ${name}@${where} created.`,
    });
    if (ok) props.onClose(true);
  };

  const footer = (
    <ModalActions
      onCancel={() => props.onClose(false)}
      onOk={() => void submit()}
      okLabel="Create…"
      disabled={!valid}
    />
  );
  return (
    <Modal title="New user" size="sm" onClose={() => props.onClose(false)} footer={footer}>
      <div className="form-grid">
        <label>User</label>
        <input
          className="input mono"
          autoFocus
          placeholder="app_user"
          value={user}
          onChange={(e) => setUser(e.target.value)}
        />
        <label>Host</label>
        <input
          className="input mono"
          placeholder="% (any host)"
          value={host}
          onChange={(e) => setHost(e.target.value)}
        />
        <label>Password</label>
        <input
          className="input"
          type="password"
          autoComplete="new-password"
          placeholder="none"
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
