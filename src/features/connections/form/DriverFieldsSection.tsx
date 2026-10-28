// The connection fields of an engine driver (SQL Server, MongoDB, Snowflake…), built from
// the driver catalog: host and port, files, driver settings (`db.extra`) and its secret.

import { Check } from "@/components/Check";
import { type DriverEntry, type DriverField } from "@/types";
import { fieldAttrs, FieldLabel, FieldNote, PasswordField, PathField } from "./fields";
import { type ConnectionFormState, parsePort, typedSecret } from "./useConnectionForm";
import { fieldValue } from "./validation";

function setField(form: ConnectionFormState, key: string, value: string) {
  const { c, setDb } = form;
  if (key === "port") return setDb({ port: parsePort(value) });
  if (key === "host" || key === "user" || key === "database" || key === "filePath") {
    return setDb({ [key]: value });
  }
  const name = key.replace(/^extra\./, "");
  const extra = { ...c.db.extra };
  if (value === "") delete extra[name];
  else extra[name] = value;
  setDb({ extra });
}

function FieldInput(props: { form: ConnectionFormState; f: DriverField; port?: number | null }) {
  const { form, f } = props;
  const value = fieldValue(form.c, f.key);
  const onChange = (v: string) => setField(form, f.key, v);
  if (f.kind === "bool") {
    return (
      <Check checked={value === "true"} onChange={(on) => onChange(on ? "true" : "")}>
        {f.help || "Enabled"}
      </Check>
    );
  }
  if (f.kind === "file") {
    return (
      <PathField
        value={value}
        onChange={onChange}
        title={`Choose ${f.label}`}
        attrs={fieldAttrs(form, f.key)}
      />
    );
  }
  if (f.kind === "select") {
    return (
      <select className="select" value={value} onChange={(e) => onChange(e.target.value)}>
        {f.options.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
    );
  }
  const placeholder = f.key === "port" ? String(props.port ?? "") : f.placeholder;
  return (
    <input
      className={`input${f.kind === "number" ? " port-input" : ""}`}
      {...fieldAttrs(form, f.key)}
      value={value}
      placeholder={placeholder || (f.required ? "required" : "")}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

/** The keychain secret: a password field, or a masked text area for pasted key files. */
function SecretField({ form, entry }: { form: ConnectionFormState; entry: DriverEntry }) {
  const { c, setDb, view, secrets, setSecret } = form;
  const secret = entry.secret;
  if (!secret) return null;
  if (!secret.multiline) {
    return (
      <PasswordField
        label={secret.label}
        mode={c.db.passwordMode}
        onMode={(passwordMode) => setDb({ passwordMode })}
        saved={!!view?.hasDbPassword}
        change={secrets.dbPassword}
        onChange={(dbPassword) => setSecret({ dbPassword })}
      />
    );
  }
  const typed = secrets.dbPassword.action === "set" ? secrets.dbPassword.value : "";
  const saved = !!view?.hasDbPassword && secrets.dbPassword.action === "keep";
  return (
    <>
      <label>{secret.label}</label>
      <textarea
        className="input mono secret-area"
        rows={4}
        spellCheck={false}
        value={typed}
        placeholder={saved ? "•••••••• saved in Keychain" : "Paste the key file's contents"}
        onChange={(e) => setSecret({ dbPassword: typedSecret(e.target.value) })}
      />
    </>
  );
}

export function DriverFieldsSection(props: { form: ConnectionFormState; entry: DriverEntry }) {
  const { form, entry } = props;
  const fields = entry.fields;
  const port = fields.find((f) => f.key === "port");
  return (
    <div className="form-grid form-section">
      {fields
        .filter((f) => f.key !== "port")
        .map((f) => (
          <FieldRow key={f.key} form={form} f={f} port={port} defaultPort={entry.defaultPort} />
        ))}
      <SecretField form={form} entry={entry} />
      {form.c.ssh.enabled && fields.some((f) => f.key === "host") && (
        <FieldNote>
          With SSH, host and port are as seen from the SSH server — usually 127.0.0.1.
        </FieldNote>
      )}
    </div>
  );
}

/** One label + input; the port shares the host's row. */
function FieldRow(props: {
  form: ConnectionFormState;
  f: DriverField;
  port: DriverField | undefined;
  defaultPort: number | null;
}) {
  const { form, f, port } = props;
  return (
    <>
      <FieldLabel form={form} field={f.key}>
        {f.label}
      </FieldLabel>
      {f.key === "host" && port ? (
        <div className="row">
          <div className="grow">
            <FieldInput form={form} f={f} />
          </div>
          <span className="muted">Port</span>
          <FieldInput form={form} f={port} port={props.defaultPort} />
        </div>
      ) : (
        <FieldInput form={form} f={f} />
      )}
      {f.help && f.kind !== "bool" && <FieldNote>{f.help}</FieldNote>}
    </>
  );
}
