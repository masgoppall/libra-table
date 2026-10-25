// SSH tunnel (SSH-1..SSH-3): server, user, password and private key.

import { KeyRound } from "lucide-react";
import { Check } from "@/components/Check";
import { FieldNote, PasswordField } from "./fields";
import { type ConnectionFormState, parsePort, typedSecret } from "./useConnectionForm";

const SAVED = "•••••••• saved in Keychain";

function passphrasePlaceholder(form: ConnectionFormState): string {
  if (form.view?.hasSshKeyPassphrase && form.secrets.sshKeyPassphrase.action === "keep") {
    return SAVED;
  }
  if (form.keyInfo?.encrypted) return "this key is encrypted — optional, asked when connecting";
  return "only for encrypted keys (optional)";
}

function KeyPicker({ form }: { form: ConnectionFormState }) {
  const { c, hasKey } = form;
  return (
    <div className="row">
      <Check checked={c.ssh.useKey} onChange={(useKey) => form.setSsh({ useKey })}>
        Use SSH key
      </Check>
      {c.ssh.useKey && (
        <button className="btn sm" onClick={form.importKey}>
          <KeyRound size={13} /> {hasKey ? "Replace key…" : "Import a private key…"}
        </button>
      )}
      {c.ssh.useKey && hasKey && (
        <>
          <span className="mono muted">{form.keyInfo?.name ?? c.ssh.keyName}</span>
          <button className="btn sm ghost" onClick={form.removeKey}>
            Remove
          </button>
        </>
      )}
    </div>
  );
}

function SshFields({ form }: { form: ConnectionFormState }) {
  const { c, setSsh, secrets, setSecret } = form;
  const passphrase = secrets.sshKeyPassphrase;
  return (
    <div className="form-grid">
      <label>Server</label>
      <div className="row">
        <input
          className="input grow"
          value={c.ssh.host}
          placeholder="bastion.example.com or ~/.ssh/config alias"
          onChange={(e) => setSsh({ host: e.target.value })}
        />
        <span className="muted">Port</span>
        <input
          className="input port-input"
          value={c.ssh.port ?? ""}
          placeholder="22"
          onChange={(e) => setSsh({ port: parsePort(e.target.value) })}
        />
      </div>
      <label>User</label>
      <input
        className="input"
        value={c.ssh.user}
        placeholder="from ~/.ssh/config or your login name"
        onChange={(e) => setSsh({ user: e.target.value })}
      />
      <PasswordField
        label="Password"
        mode={c.ssh.passwordMode}
        onMode={(passwordMode) => setSsh({ passwordMode })}
        saved={!!form.view?.hasSshPassword}
        change={secrets.sshPassword}
        onChange={(sshPassword) => setSecret({ sshPassword })}
      />
      <label>Private key</label>
      <KeyPicker form={form} />
      {c.ssh.useKey && (
        <>
          <label>Passphrase</label>
          <input
            className="input"
            type="password"
            autoComplete="new-password"
            placeholder={passphrasePlaceholder(form)}
            value={passphrase.action === "set" ? passphrase.value : ""}
            onChange={(e) => setSecret({ sshKeyPassphrase: typedSecret(e.target.value) })}
          />
        </>
      )}
      <FieldNote>
        LibraTable uses ~/.ssh/config (HostName, User, Port, IdentityFile) when the private key is
        empty. The key is copied into the Keychain.
      </FieldNote>
    </div>
  );
}

export function SshSection({ form }: { form: ConnectionFormState }) {
  const enabled = form.c.ssh.enabled;
  return (
    <div className="form-section">
      <Check
        className="form-section-title"
        checked={enabled}
        onChange={(v) => form.setSsh({ enabled: v })}
      >
        Over SSH
      </Check>
      {enabled && <SshFields form={form} />}
    </div>
  );
}
