// State and actions of the connection form. Saved passwords are never shown; typing a
// new one replaces it, "Clear" removes it from the Keychain. Missing fields turn red when
// saving or testing; errors and test results open a popup instead of a callout below.

import { homeDir } from "@tauri-apps/api/path";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/api";
import { emptySecrets } from "@/domain/connection";
import { alertError } from "@/services/errors";
import { withConnectPrompts } from "@/services/prompts";
import { store, useDriverEntry } from "@/store";
import {
  type Connection,
  type ConnectionView,
  type SecretChange,
  type SecretsInput,
  type TestResult,
} from "@/types";
import { type FieldIssue, type FieldKey, requiredFields, validateConnection } from "./validation";

export interface FormInitial {
  connection: Connection;
  view?: ConnectionView;
  /** Passwords that came from an imported URL. */
  dbPassword?: string | null;
  sshPassword?: string | null;
}

export type Busy = "test" | "save" | "connect" | null;

const SQLITE_EXTENSIONS = ["sqlite", "db", "sqlite3"];

function initialSecrets(initial: FormInitial): SecretsInput {
  const s = emptySecrets();
  if (initial.dbPassword) s.dbPassword = { action: "set", value: initial.dbPassword };
  if (initial.sshPassword) s.sshPassword = { action: "set", value: initial.sshPassword };
  return s;
}

/** A port typed in a field: empty is "default", otherwise kept within range. */
export function parsePort(v: string): number | null {
  return v.trim() === "" ? null : Math.max(0, Math.min(65535, Number(v) || 0));
}

/** An empty field keeps the saved secret; anything typed replaces it. */
export function typedSecret(value: string): SecretChange {
  return value === "" ? { action: "keep" } : { action: "set", value };
}

function fileStem(path: string): string {
  return (
    path
      .split("/")
      .pop()
      ?.replace(/\.(sqlite3?|db)$/, "") ?? ""
  );
}

/** Moves the caret to the first field still to fill in. */
function focusField(field: FieldKey) {
  // Keys are field names like "extra.accountId": no quotes to escape.
  const el = document.querySelector<HTMLElement>(`[data-field="${field}"]`);
  el?.focus();
  el?.scrollIntoView({ block: "nearest" });
}

/** The popup for missing fields; resolves once it is dismissed. */
async function reportIssues(issues: FieldIssue[]) {
  const [first] = issues;
  if (!first) return;
  await store.get().dialog({
    kind: "alert",
    tone: "warning",
    title: issues.length === 1 ? "A field needs attention" : "Some fields need attention",
    message:
      issues.length === 1 ? first.message : "Fill in the fields marked in red, then try again.",
    details: issues.length === 1 ? undefined : issues.map((i) => i.message),
  });
  focusField(first.field);
}

function testAlert(result: TestResult) {
  const warnings = result.warnings.map((w) => w.message);
  return store.get().dialog({
    kind: "alert",
    tone: warnings.length ? "warning" : "success",
    title: "Connection is OK",
    message: `Connected to ${result.serverVersion} in ${result.durationMs} ms.`,
    details: warnings,
  });
}

async function sshDirectory(): Promise<string | undefined> {
  try {
    return `${await homeDir()}/.ssh`;
  } catch {
    return undefined;
  }
}

export function useConnectionForm(initial: FormInitial) {
  const [c, setC] = useState<Connection>(initial.connection);
  const [secrets, setSecrets] = useState<SecretsInput>(() => initialSecrets(initial));
  const [keyInfo, setKeyInfo] = useState<{ name: string; encrypted: boolean } | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  /** Set by the first save or test with missing fields: from then on they show in red. */
  const [checked, setChecked] = useState(false);
  const view = initial.view;
  const entry = useDriverEntry(c.driver);
  const issues = validateConnection(c, entry);
  const required = requiredFields(c, entry);
  const invalid = (field: FieldKey) => checked && issues.some((i) => i.field === field);

  /** True when nothing is missing; otherwise marks the fields and says what to fill in. */
  const checkFields = async (ignore: FieldKey[] = []) => {
    const missing = issues.filter((i) => !ignore.includes(i.field));
    if (!missing.length) return true;
    setChecked(true);
    await reportIssues(missing);
    return false;
  };

  type Part = "db" | "ssh" | "tls" | "options";
  const setPart =
    <K extends Part>(part: K) =>
    (patch: Partial<Connection[K]>) =>
      setC((x) => ({ ...x, [part]: { ...x[part], ...patch } }));

  const set = (patch: Partial<Connection>) => setC((x) => ({ ...x, ...patch }));
  const setDb = setPart("db");
  const setSsh = setPart("ssh");
  const setTls = setPart("tls");
  const setOptions = setPart("options");
  const setSecret = (patch: Partial<SecretsInput>) => setSecrets((s) => ({ ...s, ...patch }));

  const hasKey = !!keyInfo || (!!view?.hasSshKey && !secrets.clearSshKey);

  const importKey = async () => {
    const title = "Choose SSH private key";
    const p = await open({
      multiple: false,
      directory: false,
      defaultPath: await sshDirectory(),
      title,
    });
    if (typeof p !== "string") return;
    try {
      const info = await api.importSshKey(p);
      setKeyInfo({ name: info.name, encrypted: info.encrypted });
      setSecret({ sshKeyToken: info.token, clearSshKey: false });
      setSsh({ keyName: info.name, useKey: true });
    } catch (e) {
      await alertError("Couldn't import the key", e);
    }
  };

  const removeKey = () => {
    setKeyInfo(null);
    setSecret({ sshKeyToken: null, clearSshKey: true });
    setSsh({ keyName: "" });
  };

  /** Creates an empty SQLite file and points the form at it. */
  const createSqliteFile = async () => {
    const p = await saveDialog({
      title: "New SQLite database",
      defaultPath: `${(c.name || "database").replace(/[^\w.-]+/g, "_")}.sqlite`,
      filters: [{ name: "SQLite", extensions: SQLITE_EXTENSIONS }],
    });
    if (!p) return;
    try {
      await api.createSqliteFile(p);
      setDb({ filePath: p });
      if (!c.name) set({ name: fileStem(p) });
    } catch (e) {
      await alertError("Couldn't create the database file", e);
    }
  };

  /** Test with prompts for host key, passphrase and "ask" passwords. A name isn't needed. */
  const runTest = async () => {
    if (!(await checkFields(["name"]))) return;
    setBusy("test");
    let result: TestResult | null;
    try {
      result = await withConnectPrompts((p) => api.testConnection(c, secrets, p), {
        title: c.name || "Test",
        offerSavePassphrase: false,
      });
    } catch (e) {
      setBusy(null);
      await alertError("Couldn't connect", e, "connection-failed");
      return;
    }
    setBusy(null);
    // Null when a prompt (password, host key) was cancelled.
    if (result) await testAlert(result);
  };

  const save = async (andConnect: boolean): Promise<ConnectionView | null> => {
    if (!(await checkFields())) return null;
    setBusy(andConnect ? "connect" : "save");
    try {
      const saved = await api.saveConnection(c, secrets);
      setBusy(null);
      return saved;
    } catch (e) {
      setBusy(null);
      await alertError("Couldn't save the connection", e);
      return null;
    }
  };

  return {
    c,
    view,
    secrets,
    keyInfo,
    hasKey,
    busy,
    required,
    invalid,
    set,
    setDb,
    setSsh,
    setTls,
    setOptions,
    setSecret,
    importKey,
    removeKey,
    createSqliteFile,
    runTest,
    save,
  };
}

export type ConnectionFormState = ReturnType<typeof useConnectionForm>;
