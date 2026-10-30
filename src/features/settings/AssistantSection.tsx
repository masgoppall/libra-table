// Settings › SQL assistant: turning it on (with what it sends), the model, and the
// Anthropic API key, which Rust keeps in the Keychain.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { Check } from "@/components/Check";
import { showError, toastSuccess } from "@/services/errors";
import { type AiModel } from "@/types";
import { type SectionProps } from "./SettingsSections";

const MODELS: [AiModel, string][] = [
  ["claude-opus-5-5", "Claude Opus 5.5 (best results)"],
  ["claude-sonnet-5-5", "Claude Sonnet 5.5 (faster)"],
  ["claude-haiku-5-5", "Claude Haiku 5.5 (fastest, cheapest)"],
];

function KeyField() {
  const [saved, setSaved] = useState<boolean | null>(null);
  const [key, setKey] = useState("");
  useEffect(() => {
    api
      .aiKeyStatus()
      .then(setSaved)
      .catch(() => setSaved(false));
  }, []);
  const save = async () => {
    try {
      await api.aiSetKey(key);
      setKey("");
      setSaved(true);
      toastSuccess("API key saved in the Keychain.");
    } catch (e) {
      showError(e);
    }
  };
  const remove = async () => {
    try {
      await api.aiClearKey();
      setSaved(false);
    } catch (e) {
      showError(e);
    }
  };
  return (
    <div className="row">
      <input
        className="input grow"
        type="password"
        aria-label="Anthropic API key"
        placeholder={saved ? "Saved in the Keychain" : "sk-ant-…"}
        value={key}
        autoComplete="off"
        onChange={(e) => setKey(e.target.value)}
      />
      <button className="btn sm" disabled={!key.trim()} onClick={() => void save()}>
        Save key
      </button>
      {saved && (
        <button className="btn sm ghost" onClick={() => void remove()}>
          Remove
        </button>
      )}
    </div>
  );
}

export function AssistantSection({ s, set }: SectionProps) {
  return (
    <>
      <div className="settings-section">SQL assistant</div>
      <div className="form-grid">
        <label>Assistant</label>
        <div className="col">
          <Check checked={s.aiEnabled} onChange={(aiEnabled) => set({ aiEnabled })}>
            Write queries from questions in plain language
          </Check>
          <span className="faint small">
            When you ask, the question, the table and column names of the database and the query in
            the editor are sent to Anthropic. Rows are never sent.
          </span>
        </div>
        {s.aiEnabled && (
          <>
            <label>Model</label>
            <select
              className="select"
              value={s.aiModel}
              onChange={(e) => set({ aiModel: e.target.value as AiModel })}
            >
              {MODELS.map(([m, label]) => (
                <option key={m} value={m}>
                  {label}
                </option>
              ))}
            </select>
            <label>API key</label>
            <KeyField />
          </>
        )}
      </div>
    </>
  );
}
