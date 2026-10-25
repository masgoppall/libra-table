// Connection form (CON-3..CON-7, SSH-1..SSH-3, TLS-1).

import { CheckCircle2 } from "lucide-react";
import { DriverLogo } from "@/components/DriverLogo";
import { ErrorCallout } from "@/components/ErrorCallout";
import { Modal } from "@/components/Modal";
import { BusyIcon } from "@/components/Spinner";
import { builtInDialect, DRIVER_LABEL } from "@/domain/connection";
import { useDriverEntry } from "@/store";
import { type ConnectionView, type TestResult } from "@/types";
import { DriverFieldsSection } from "./DriverFieldsSection";
import { GeneralSection } from "./GeneralSection";
import { OptionsSection } from "./OptionsSection";
import { SafetySection } from "./SafetySection";
import { ServerSection } from "./ServerSection";
import { SqliteSection } from "./SqliteSection";
import { SshSection } from "./SshSection";
import { TlsSection } from "./TlsSection";
import { type ConnectionFormState, type FormInitial, useConnectionForm } from "./useConnectionForm";

function TestOutcome({ test }: { test: TestResult | null }) {
  if (!test) return null;
  return (
    <div className="col">
      <div className="callout ok">
        <CheckCircle2 size={13} className="inline-icon" /> Connected to {test.serverVersion} in{" "}
        {test.durationMs} ms.
      </div>
      {test.warnings.map((w) => (
        <div key={w.code} className="callout warn">
          {w.message}
        </div>
      ))}
    </div>
  );
}

function Footer(props: {
  form: ConnectionFormState;
  onClose: () => void;
  onSave: (andConnect: boolean) => void;
}) {
  const { busy, runTest } = props.form;
  return (
    <>
      <button className="btn" onClick={runTest} disabled={!!busy}>
        <BusyIcon busy={busy === "test"} icon={null} size={14} /> Test
      </button>
      <span className="spacer" />
      <button className="btn" onClick={props.onClose}>
        Cancel
      </button>
      <button className="btn" onClick={() => props.onSave(false)} disabled={!!busy}>
        <BusyIcon busy={busy === "save"} icon={null} size={14} /> Save
      </button>
      <button className="btn primary" onClick={() => props.onSave(true)} disabled={!!busy}>
        <BusyIcon busy={busy === "connect"} icon={null} size={14} /> Connect
      </button>
    </>
  );
}

export function ConnectionForm(props: {
  initial: FormInitial;
  onClose: () => void;
  onSaved: (v: ConnectionView, connect: boolean) => void;
}) {
  const form = useConnectionForm(props.initial);
  const { c } = form;
  const entry = useDriverEntry(c.driver);
  const dialect = builtInDialect(c.driver);
  const isSqlite = dialect === "sqlite";
  // The built-in drivers have their own sections; engines are described by the catalog.
  const ssh = dialect ? !isSqlite : !!entry?.ssh;
  const tls = dialect ? !isSqlite : !!entry?.tls;
  const onSave = async (andConnect: boolean) => {
    const saved = await form.save(andConnect);
    if (saved) props.onSaved(saved, andConnect);
  };
  return (
    <Modal
      size="lg"
      title={`${DRIVER_LABEL[c.driver]} Connection`}
      icon={<DriverLogo driver={c.driver} size={22} />}
      onClose={props.onClose}
      footer={<Footer form={form} onClose={props.onClose} onSave={onSave} />}
    >
      <GeneralSection form={form} />
      {isSqlite && <SqliteSection form={form} />}
      {dialect && !isSqlite && <ServerSection form={form} />}
      {!dialect && entry && <DriverFieldsSection form={form} entry={entry} />}
      {ssh && <SshSection form={form} />}
      {tls && <TlsSection form={form} />}
      <OptionsSection form={form} />
      <SafetySection form={form} />
      <TestOutcome test={form.test} />
      <ErrorCallout error={form.error} />
    </Modal>
  );
}
