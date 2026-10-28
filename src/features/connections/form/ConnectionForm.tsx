// Connection form (CON-3..CON-7, SSH-1..SSH-3, TLS-1). Required fields are marked; missing
// ones turn red on save, and errors and test results open a popup.

import { DriverCircle } from "@/components/DriverLogo";
import { Modal } from "@/components/Modal";
import { BusyIcon } from "@/components/Spinner";
import { builtInDialect, DRIVER_LABEL } from "@/domain/connection";
import { useDriverEntry } from "@/store";
import { type ConnectionView } from "@/types";
import { DriverFieldsSection } from "./DriverFieldsSection";
import { GeneralSection } from "./GeneralSection";
import { OptionsSection } from "./OptionsSection";
import { SafetySection } from "./SafetySection";
import { ServerSection } from "./ServerSection";
import { SqliteSection } from "./SqliteSection";
import { SshSection } from "./SshSection";
import { TlsSection } from "./TlsSection";
import { type ConnectionFormState, type FormInitial, useConnectionForm } from "./useConnectionForm";

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
      icon={<DriverCircle driver={c.driver} size={24} />}
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
    </Modal>
  );
}
