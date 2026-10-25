// Export connections to a file and import them on another Mac. Secrets travel only
// encrypted with a passphrase the user chooses.

import { open, save } from "@tauri-apps/plugin-dialog";
import { api } from "@/api";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";

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

export async function importConnections() {
  const path = await open({ multiple: false, directory: false, filters: FILTERS });
  if (typeof path !== "string") return;
  try {
    const summary = await api.inspectConnectionsFile(path);
    const passphrase = await importPassphrase(summary.hasSecrets);
    if (passphrase === undefined) return;
    const n = await api.importConnections(path, passphrase);
    await store.get().loadConnections();
    toastSuccess(`Imported ${plural(n, "connection")}.`);
  } catch (e) {
    showError(e);
  }
}
