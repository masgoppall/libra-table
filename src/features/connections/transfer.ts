// Export connections to a file and import them on another Mac. Secrets travel only
// encrypted with a passphrase the user chooses. TablePlus exports import the same way.

import { open, save } from "@tauri-apps/plugin-dialog";
import { api } from "@/api";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type AppError, type ImportReport } from "@/types";

const FILTERS = [{ name: "LibraTable connections", extensions: ["json"] }];

/** Null = cancelled; "" = export without secrets. */
async function askPassphrase(): Promise<string | null> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Include passwords?",
    message:
      "Leave empty to export without passwords and keys. With a passphrase they are " +
      "included, encrypted (AES-256).",
    label: "Passphrase (optional)",
    secret: true,
    okLabel: "Continue",
  });
  if (!r.ok) return null;
  const pass = r.value ?? "";
  if (!pass) return "";
  const again = await store.get().dialog({
    kind: "prompt",
    title: "Confirm passphrase",
    label: "Type the passphrase again",
    secret: true,
  });
  if (!again.ok) return null;
  if (again.value !== pass) {
    showError({ message: "The passphrases don't match." });
    return null;
  }
  return pass;
}

/** Exports the given connections (all when `ids` is empty). */
export async function exportConnections(ids: string[], defaultName = "connections") {
  const passphrase = await askPassphrase();
  if (passphrase === null) return;
  const path = await save({ defaultPath: `${defaultName}.json`, filters: FILTERS });
  if (!path) return;
  try {
    const n = await api.exportConnections(ids, path, passphrase || null);
    const what = passphrase ? "with encrypted passwords" : "without passwords";
    toastSuccess(`Exported ${plural(n, "connection")} ${what}.`, path);
  } catch (e) {
    showError(e);
  }
}

async function importPassphrase(hasSecrets: boolean): Promise<string | null | undefined> {
  if (!hasSecrets) return null;
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Passwords are included",
    message: "Type the passphrase used for the export, or leave empty to skip the passwords.",
    label: "Passphrase",
    secret: true,
    okLabel: "Import",
  });
  return r.ok ? r.value || null : undefined;
}

/** Where the import starts: any connections file, or a TablePlus export. */
export type ImportSource = "any" | "tableplus";

const TABLEPLUS_EXTENSION = "tableplusconnection";
const TABLEPLUS_FILTERS = [{ name: "TablePlus connections", extensions: [TABLEPLUS_EXTENSION] }];
const IMPORT_FILTERS = [
  { name: "Connections (LibraTable, TablePlus)", extensions: ["json", TABLEPLUS_EXTENSION] },
];
/** Start of the backend's error for a password that doesn't open the file. */
const WRONG_PASSWORD = "The password is wrong";
const ALREADY_THERE = "already in LibraTable";

/** Null = cancelled. TablePlus encrypts the whole file, so the password is required. */
async function tablePlusPassword(retry: boolean): Promise<string | null> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: retry ? "Wrong password" : "Import from TablePlus",
    message: retry
      ? "That password doesn't open this file. Type the password you chose when " +
        "exporting from TablePlus."
      : "Type the password you chose when exporting from TablePlus. Connections, groups, " +
        "passwords and SSH keys come over; connections you already have are left alone.",
    label: "Export password",
    secret: true,
    okLabel: "Import",
  });
  return r.ok && r.value ? r.value : null;
}

function isWrongPassword(e: unknown): boolean {
  return String((e as Partial<AppError> | undefined)?.message ?? "").startsWith(WRONG_PASSWORD);
}

async function importTablePlus(path: string): Promise<ImportReport | null> {
  for (let retry = false; ; retry = true) {
    const password = await tablePlusPassword(retry);
    if (password === null) return null;
    try {
      return await api.importConnections(path, password);
    } catch (e) {
      if (!isWrongPassword(e)) throw e;
    }
  }
}

/** A toast when everything came over; a popup listing what didn't otherwise. */
async function reportImport(report: ImportReport, from: string) {
  const known = report.skipped.filter((s) => s.reason === ALREADY_THERE).length;
  const others = report.skipped.filter((s) => s.reason !== ALREADY_THERE);
  const added = `Imported ${plural(report.added, "connection")}${from}.`;
  const kept = known ? `${plural(known, "connection")} you already had stayed as they were.` : "";
  if (!others.length) {
    toastSuccess(report.added || !known ? added : "Nothing new to import.", kept || undefined);
    return;
  }
  await store.get().dialog({
    kind: "alert",
    tone: report.added ? "warning" : "error",
    title: added,
    message: `${plural(others.length, "connection")} couldn't be imported:`,
    details: others.map((s) => `${s.name || "Unnamed"}: ${s.reason}`),
    hint: kept || undefined,
  });
}

export async function importConnections(source: ImportSource = "any") {
  const path = await open({
    multiple: false,
    directory: false,
    title: source === "tableplus" ? "Choose the TablePlus export" : undefined,
    filters: source === "tableplus" ? TABLEPLUS_FILTERS : IMPORT_FILTERS,
  });
  if (typeof path !== "string") return;
  try {
    const summary = await api.inspectConnectionsFile(path);
    let report: ImportReport | null;
    if (summary.source === "tableplus") {
      report = await importTablePlus(path);
    } else {
      const passphrase = await importPassphrase(summary.hasSecrets);
      report = passphrase === undefined ? null : await api.importConnections(path, passphrase);
    }
    if (!report) return;
    await store.get().loadConnections();
    await reportImport(report, summary.source === "tableplus" ? " from TablePlus" : "");
  } catch (e) {
    showError(e);
  }
}
