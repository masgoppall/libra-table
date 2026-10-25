// Pure rules of the process list: refresh intervals, search, idle rows, durations and
// the wording of the stop confirmations.

import { type Dialect, type ProcessAction, type ProcessInfo } from "@/types";

/** Milliseconds between refreshes; 0 is off. */
export type RefreshInterval = 0 | 2000 | 5000 | 10000;

export const REFRESH_OPTIONS: [RefreshInterval, string][] = [
  [0, "Auto-refresh off"],
  [2000, "Every 2 s"],
  [5000, "Every 5 s"],
  [10000, "Every 10 s"],
];

/** Postgres "idle", MySQL "Sleep": connected but doing nothing. */
export function isIdle(p: ProcessInfo): boolean {
  return p.state === "idle" || p.state === "Sleep";
}

export function filterProcesses(
  list: ProcessInfo[],
  text: string,
  hideIdle: boolean,
): ProcessInfo[] {
  const needle = text.trim().toLowerCase();
  return list.filter((p) => {
    if (hideIdle && isIdle(p)) return false;
    if (!needle) return true;
    const hay = [p.id, p.user, p.database, p.client, p.application, p.state, p.query];
    return hay.some((v) => String(v).toLowerCase().includes(needle));
  });
}

/** "850 ms", "12 s", "3m 05s", "2h 10m", "3d 4h"; "—" when unknown. */
export function formatAge(ms: number | null): string {
  if (ms === null || ms < 0) return "—";
  if (ms < 1000) return `${ms} ms`;
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, "0")}s`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${String(m % 60).padStart(2, "0")}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

export interface StopWording {
  title: string;
  message: string;
  okLabel: string;
  /** LibraTable's own connection: the user types the id to go ahead. */
  typeToConfirm?: string;
}

export function stopWording(driver: Dialect, p: ProcessInfo, action: ProcessAction): StopWording {
  const what = driver === "postgres" ? `process ${p.id}` : `connection ${p.id}`;
  const who = p.user ? ` (${p.user}${p.database ? ` on ${p.database}` : ""})` : "";
  const base =
    action === "cancel"
      ? `Cancel the query running on ${what}${who}? The connection stays open.`
      : `Close ${what}${who}? Its running query and open transaction are rolled back.`;
  const own = p.own
    ? " This is LibraTable's own connection: stopping it interrupts what you're doing here " +
      "(LibraTable reconnects on the next query)."
    : "";
  return {
    title: action === "cancel" ? "Cancel query" : "Kill connection",
    message: base + own,
    okLabel: action === "cancel" ? "Cancel query" : "Kill connection",
    ...(p.own ? { typeToConfirm: String(p.id) } : {}),
  };
}
