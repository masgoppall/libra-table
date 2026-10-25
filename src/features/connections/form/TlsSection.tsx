// TLS-1: SSL mode and certificates. The modes and their names differ per driver.

import { type Driver, type TlsMode } from "@/types";
import { PathField } from "./fields";
import { type ConnectionFormState } from "./useConnectionForm";

const TLS_MODES: Partial<Record<Driver, [TlsMode, string][]>> = {
  postgres: [
    ["disable", "Disable"],
    ["allow", "Allow"],
    ["prefer", "Prefer (default)"],
    ["require", "Require"],
    ["verify_ca", "Verify CA"],
    ["verify_full", "Verify full"],
  ],
  mysql: [
    ["disable", "Disabled"],
    ["prefer", "Preferred (default)"],
    ["require", "Required"],
    ["verify_ca", "Verify CA"],
    ["verify_full", "Verify identity"],
  ],
};

export function TlsSection({ form }: { form: ConnectionFormState }) {
  const { c, setTls } = form;
  return (
    <div className="form-grid form-section">
      <label>SSL mode</label>
      <select
        className="select"
        value={c.tls.mode}
        onChange={(e) => setTls({ mode: e.target.value as TlsMode })}
      >
        {(TLS_MODES[c.driver] ?? []).map(([mode, label]) => (
          <option key={mode} value={mode}>
            {label}
          </option>
        ))}
      </select>
      {c.tls.mode !== "disable" && (
        <>
          <label>CA certificate</label>
          <PathField
            value={c.tls.caPath}
            onChange={(caPath) => setTls({ caPath })}
            placeholder="optional"
            title="Choose CA certificate"
          />
          <label>Client certificate</label>
          <PathField
            value={c.tls.certPath}
            onChange={(certPath) => setTls({ certPath })}
            placeholder="optional"
            title="Choose client certificate"
          />
          <label>Client key</label>
          <PathField
            value={c.tls.keyPath}
            onChange={(keyPath) => setTls({ keyPath })}
            placeholder="optional"
            title="Choose client key"
          />
        </>
      )}
    </div>
  );
}
