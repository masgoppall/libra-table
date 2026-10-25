// Safe mode levels (TablePlus "safe mode"): when the app asks before statements run.

import { type Connection, type SafeMode } from "@/types";

export interface SafeModeInfo {
  id: SafeMode;
  label: string;
  description: string;
}

export const SAFE_MODES: SafeModeInfo[] = [
  {
    id: "auto",
    label: "Automatic",
    description: "Confirm statements that change data when the tag is production.",
  },
  { id: "silent", label: "Silent", description: "Never ask before running statements." },
  {
    id: "alert_writes",
    label: "Alert before writes",
    description: "Confirm statements that change data or structure.",
  },
  { id: "alert_all", label: "Alert before every query", description: "Confirm every statement." },
  {
    id: "safe_writes",
    label: "Safe: writes",
    description: "Type the connection name to run statements that change data.",
  },
  {
    id: "safe_all",
    label: "Safe: every query",
    description: "Type the connection name to run any statement.",
  },
];

/** How a change to data or structure is confirmed on this connection. */
export type WriteConfirmation = "none" | "alert" | "safe";

export function writeConfirmation(c: Pick<Connection, "tag" | "options">): WriteConfirmation {
  switch (c.options.safeMode) {
    case "auto":
      return c.tag === "production" ? "alert" : "none";
    case "silent":
      return "none";
    case "alert_writes":
    case "alert_all":
      return "alert";
    case "safe_writes":
    case "safe_all":
      return "safe";
  }
}

export const KEEP_ALIVE_CHOICES: [number, string][] = [
  [0, "Off"],
  [30, "Every 30 seconds"],
  [60, "Every minute"],
  [300, "Every 5 minutes"],
];
