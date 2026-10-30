// Runs due backup schedules every minute while LibraTable is open (main window only).

import { useEffect } from "react";
import { runDueSchedules } from "@/features/server/backup/backupRunner";
import { isConnectionWindow } from "@/lib/windowRole";
import { useStore } from "@/store";

const CHECK_MS = 60_000;

export function useBackupScheduler() {
  const any = useStore((s) => (s.settings?.backupSchedules ?? []).some((b) => b.enabled));
  useEffect(() => {
    if (isConnectionWindow || !any) return;
    const t = setInterval(() => void runDueSchedules(), CHECK_MS);
    const first = setTimeout(() => void runDueSchedules(), 5_000);
    return () => {
      clearInterval(t);
      clearTimeout(first);
    };
  }, [any]);
}
