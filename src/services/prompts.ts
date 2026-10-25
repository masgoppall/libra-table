// Connecting may need answers: a password set to "ask", a key passphrase, or trust in
// an unknown SSH host key (SSH-4). This asks, then the caller retries with the answers.

import { api } from "@/api";
import { store } from "@/store";
import {
  type AppError,
  type HostKeyError,
  type PassphraseError,
  type PasswordRequiredError,
  type PromptedSecrets,
} from "@/types";

const MAX_ATTEMPTS = 6;

export interface PromptOptions {
  /** Dialog title for password prompts (the connection name). */
  title: string;
  /** Offer "Save in Keychain" for a key passphrase (not while testing). */
  offerSavePassphrase: boolean;
}

type Outcome = "retry" | "cancelled" | "unhandled";

async function askPassword(
  e: PasswordRequiredError,
  prompted: PromptedSecrets,
  opts: PromptOptions,
): Promise<Outcome> {
  const which = e.data?.which === "ssh" ? "SSH" : "database";
  const r = await store.get().dialog({
    kind: "prompt",
    title: opts.title,
    message: `Enter the ${which} password. It is not saved.`,
    label: "Password",
    secret: true,
  });
  if (!r.ok) return "cancelled";
  if (which === "SSH") prompted.sshPassword = r.value ?? "";
  else prompted.dbPassword = r.value ?? "";
  return "retry";
}

async function askPassphrase(
  e: PassphraseError,
  prompted: PromptedSecrets,
  opts: PromptOptions,
): Promise<Outcome> {
  const wrong = e.kind === "ssh_passphrase_wrong";
  const keyName = e.data?.keyName ?? "the private key";
  const r = await store.get().dialog({
    kind: "prompt",
    title: wrong ? "Wrong passphrase" : "Private key passphrase",
    message: wrong ? `${e.message} ${e.hint ?? ""}` : `Enter the passphrase for ${keyName}.`,
    label: "Passphrase",
    secret: true,
    checkbox: opts.offerSavePassphrase ? "Save in Keychain" : undefined,
  });
  if (!r.ok) return "cancelled";
  prompted.sshKeyPassphrase = r.value ?? "";
  prompted.savePassphrase = !!r.checked;
  return "retry";
}

async function askHostKey(e: HostKeyError): Promise<Outcome> {
  const r = await store.get().dialog({ kind: "hostKey", error: e });
  if (!r.ok || !e.data) return "cancelled";
  await api.trustHostKey(e.data.host, e.data.port);
  return "retry";
}

function answer(e: AppError, prompted: PromptedSecrets, opts: PromptOptions): Promise<Outcome> {
  switch (e.kind) {
    case "password_required":
      return askPassword(e, prompted, opts);
    case "ssh_passphrase_required":
    case "ssh_passphrase_wrong":
      return askPassphrase(e, prompted, opts);
    case "ssh_host_key_unknown":
    case "ssh_host_key_changed":
      return askHostKey(e);
    default:
      return Promise.resolve("unhandled");
  }
}

/**
 * Runs `attempt`, answering prompts as they come and retrying with the answers.
 * Resolves to null when the user cancels a prompt; other errors are thrown.
 */
export async function withConnectPrompts<T>(
  attempt: (prompted: PromptedSecrets) => Promise<T>,
  opts: PromptOptions,
): Promise<T | null> {
  const prompted: PromptedSecrets = {};
  for (let i = 0; i < MAX_ATTEMPTS; i++) {
    try {
      return await attempt(prompted);
    } catch (err) {
      const outcome = await answer(err as AppError, prompted, opts);
      if (outcome === "cancelled") return null;
      if (outcome === "unhandled") throw err;
    }
  }
  return null;
}
