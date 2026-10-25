// Passcode lock settings: set, change or remove the passcode; lock after idle time.

import { Lock } from "lucide-react";
import { api } from "@/api";
import { showError, toastSuccess } from "@/services/errors";
import { store, useStore } from "@/store";
import { type SectionProps } from "./SettingsSections";

const LOCK_AFTER: [number, string][] = [
  [0, "Only when LibraTable starts"],
  [1, "After 1 minute idle"],
  [5, "After 5 minutes idle"],
  [15, "After 15 minutes idle"],
  [30, "After 30 minutes idle"],
  [60, "After 1 hour idle"],
];

async function ask(title: string, label: string): Promise<string | null> {
  const r = await store.get().dialog({ kind: "prompt", title, label, secret: true });
  return r.ok ? (r.value ?? "") : null;
}

/** Asks for the current passcode (if any), then the new one twice. */
async function setPasscode(enabled: boolean) {
  const current = enabled ? await ask("Change passcode", "Current passcode") : null;
  if (enabled && current === null) return;
  const next = await ask(enabled ? "Change passcode" : "Set a passcode", "New passcode");
  if (!next) return;
  const again = await ask("Confirm passcode", "Type the new passcode again");
  if (again === null) return;
  if (again !== next) return showError({ message: "The passcodes don't match." });
  try {
    await api.changePasscode(current, next);
    store.get().setPasscodeEnabled(true);
    toastSuccess("Passcode saved. LibraTable locks when it starts.");
  } catch (e) {
    showError(e);
  }
}

async function removePasscode() {
  const current = await ask("Remove passcode", "Current passcode");
  if (current === null) return;
  try {
    await api.changePasscode(current, null);
    store.get().setPasscodeEnabled(false);
    toastSuccess("Passcode removed.");
  } catch (e) {
    showError(e);
  }
}

export function SecuritySection({ s, set }: SectionProps) {
  const enabled = useStore((st) => st.passcodeEnabled);
  return (
    <>
      <div className="settings-section">Security</div>
      <div className="form-grid">
        <label>Passcode</label>
        <div className="row">
          <button className="btn sm" onClick={() => void setPasscode(enabled)}>
            <Lock size={13} /> {enabled ? "Change…" : "Set passcode…"}
          </button>
          {enabled && (
            <>
              <button className="btn sm ghost" onClick={() => void removePasscode()}>
                Remove…
              </button>
              <button className="btn sm ghost" onClick={() => store.get().setLocked(true)}>
                Lock now
              </button>
            </>
          )}
        </div>
        <label>Lock</label>
        <select
          className="select"
          disabled={!enabled}
          value={s.lockAfterMinutes}
          onChange={(e) => set({ lockAfterMinutes: Number(e.target.value) })}
        >
          {LOCK_AFTER.map(([m, label]) => (
            <option key={m} value={m}>
              {label}
            </option>
          ))}
        </select>
      </div>
    </>
  );
}
