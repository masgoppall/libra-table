// Where the database server is: host and port (or a socket), user, password, database.

import { Check } from "@/components/Check";
import { builtInDialect, DEFAULT_PORT } from "@/domain/connection";
import { fieldAttrs, FieldLabel, FieldNote, PasswordField } from "./fields";
import { type ConnectionFormState, parsePort } from "./useConnectionForm";

function HostRow({ form }: { form: ConnectionFormState }) {
  const { c, setDb } = form;
  if (c.db.useSocket) {
    return (
      <input
        className="input mono grow"
        {...fieldAttrs(form, "socket")}
        value={c.db.socket}
        placeholder="/tmp/.s.PGSQL.5432"
        onChange={(e) => setDb({ socket: e.target.value })}
      />
    );
  }
  return (
    <>
      <input
        className="input grow"
        value={c.db.host}
        placeholder="127.0.0.1"
        onChange={(e) => setDb({ host: e.target.value })}
      />
      <span className="muted">Port</span>
      <input
        className="input port-input"
        value={c.db.port ?? ""}
        placeholder={String(DEFAULT_PORT[c.driver] ?? "")}
        onChange={(e) => setDb({ port: parsePort(e.target.value) })}
      />
    </>
  );
}

export function ServerSection({ form }: { form: ConnectionFormState }) {
  const { c, setDb, view, secrets, setSecret } = form;
  return (
    <div className="form-grid form-section">
      <FieldLabel form={form} field={c.db.useSocket ? "socket" : "host"}>
        {c.db.useSocket ? "Socket" : "Host"}
      </FieldLabel>
      <div className="row">
        <HostRow form={form} />
        <Check
          checked={c.db.useSocket}
          disabled={c.ssh.enabled}
          onChange={(useSocket) => setDb({ useSocket })}
          title="Connect through a local socket file"
        >
          Socket
        </Check>
      </div>
      <label>User</label>
      <input
        className="input"
        value={c.db.user}
        placeholder={builtInDialect(c.driver) === "postgres" ? "postgres" : "root"}
        onChange={(e) => setDb({ user: e.target.value })}
      />
      <PasswordField
        label="Password"
        mode={c.db.passwordMode}
        onMode={(passwordMode) => setDb({ passwordMode })}
        saved={!!view?.hasDbPassword}
        change={secrets.dbPassword}
        onChange={(dbPassword) => setSecret({ dbPassword })}
      />
      <label>Database</label>
      <input
        className="input"
        value={c.db.database}
        placeholder="database name (optional)"
        onChange={(e) => setDb({ database: e.target.value })}
      />
      {c.ssh.enabled && (
        <FieldNote>
          With SSH, host and port are as seen from the SSH server — usually 127.0.0.1.
        </FieldNote>
      )}
    </div>
  );
}
