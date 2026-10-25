import { Keyboard, Settings as SettingsIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "@/api";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { showError } from "@/services/errors";
import { store, useSettings } from "@/store";
import { type AppInfo, type Settings } from "@/types";
import { SecuritySection } from "./SecuritySection";
import { AppearanceSection, FiltersSection, HistorySection } from "./SettingsSections";

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const settings = useSettings();
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api
      .appInfo()
      .then(setInfo)
      .catch(() => undefined);
  }, []);
  if (!draft) return null;

  const set = (patch: Partial<Settings>) => setDraft({ ...draft, ...patch });
  const save = async () => {
    try {
      await store.get().saveSettings(draft);
      onClose();
    } catch (e) {
      showError(e);
    }
  };

  return (
    <Modal
      title="Settings"
      icon={<SettingsIcon size={18} />}
      onClose={onClose}
      footer={
        <ModalActions
          leading={<span className="faint small">{info ? `LibraTable ${info.version}` : ""}</span>}
          onCancel={onClose}
          onOk={() => void save()}
          okLabel="Save"
        />
      }
    >
      <AppearanceSection s={draft} set={set} />
      <FiltersSection s={draft} set={set} />
      <HistorySection s={draft} set={set} info={info} />
      <SecuritySection s={draft} set={set} />
      <div className="row settings-foot">
        <button className="btn sm ghost" onClick={() => store.get().setShortcutsOpen(true)}>
          <Keyboard size={13} /> Keyboard shortcuts
        </button>
        <span className="spacer" />
        <span className="faint small">Passwords and keys live only in the system Keychain.</span>
      </div>
    </Modal>
  );
}
