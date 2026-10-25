// The lock screen (passcode lock): covers the whole window until the passcode is typed.

import { Lock } from "lucide-react";
import { useState } from "react";
import { api } from "@/api";
import { Spinner } from "@/components/Spinner";
import { showError } from "@/services/errors";
import { store, useStore } from "@/store";

export function LockScreen() {
  const locked = useStore((s) => s.locked);
  const [code, setCode] = useState("");
  const [wrong, setWrong] = useState(false);
  const [busy, setBusy] = useState(false);
  if (!locked) return null;

  const unlock = async () => {
    if (!code || busy) return;
    setBusy(true);
    try {
      const ok = await api.verifyPasscode(code);
      setWrong(!ok);
      setCode("");
      if (ok) store.get().setLocked(false);
    } catch (e) {
      showError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="lock-screen" role="dialog" aria-modal="true" aria-label="LibraTable is locked">
      <form
        className="lock-card"
        onSubmit={(e) => {
          e.preventDefault();
          void unlock();
        }}
      >
        <Lock size={28} className="lock-icon" />
        <h2>LibraTable is locked</h2>
        <input
          className="input lock-input"
          type="password"
          autoFocus
          autoComplete="off"
          placeholder="Passcode"
          value={code}
          onChange={(e) => {
            setCode(e.target.value);
            setWrong(false);
          }}
        />
        {wrong && <div className="warn-text small">Wrong passcode.</div>}
        <button className="btn primary" type="submit" disabled={!code || busy}>
          {busy && <Spinner />} Unlock
        </button>
      </form>
    </div>
  );
}
