// Locks the app at start (when a passcode is set) and after the idle time in Settings.

import { useEffect, useRef } from "react";
import { api } from "@/api";
import { store, useSettings, useStore } from "@/store";

const ACTIVITY = ["mousedown", "keydown", "wheel", "mousemove"] as const;
const CHECK_MS = 15_000;

export function usePasscodeLock() {
  const enabled = useStore((s) => s.passcodeEnabled);
  const minutes = useSettings()?.lockAfterMinutes ?? 0;
  const lastActive = useRef(Date.now());

  useEffect(() => {
    api
      .passcodeSet()
      .then((set) => {
        store.get().setPasscodeEnabled(set);
        if (set) store.get().setLocked(true);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!enabled || minutes <= 0) return;
    const touch = () => (lastActive.current = Date.now());
    ACTIVITY.forEach((ev) => window.addEventListener(ev, touch, { passive: true }));
    const timer = setInterval(() => {
      const idle = Date.now() - lastActive.current;
      if (idle >= minutes * 60_000 && !store.get().locked) store.get().setLocked(true);
    }, CHECK_MS);
    return () => {
      ACTIVITY.forEach((ev) => window.removeEventListener(ev, touch));
      clearInterval(timer);
    };
  }, [enabled, minutes]);
}
