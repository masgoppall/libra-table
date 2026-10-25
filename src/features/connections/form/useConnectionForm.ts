// State and actions of the connection form. Saved passwords are never shown; typing a
// new one replaces it, "Clear" removes it from the Keychain.

import { homeDir } from "@tauri-apps/api/path";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/api";
import { emptySecrets } from "@/domain/connection";
import { withConnectPrompts } from "@/services/prompts";
import {
  type AppError,
  type Connection,
  type ConnectionView,
  type SecretChange,
  type SecretsInput,
  type TestResult,
} from "@/types";

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
  const [test, setTest] = useState<TestResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const view = initial.view;

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
      setError(e as AppError);
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
      setError(null);
    } catch (e) {
      setError(e as AppError);
    }
  };

  /** Test with prompts for host key, passphrase and "ask" passwords. */
  const runTest = async () => {
    setBusy("test");
    setError(null);
    setTest(null);
    try {
      const result = await withConnectPrompts((p) => api.testConnection(c, secrets, p), {
        title: c.name || "Test",
        offerSavePassphrase: false,
      });
      setTest(result);
    } catch (e) {
      setError(e as AppError);
    } finally {
      setBusy(null);
    }
  };

  const save = async (andConnect: boolean): Promise<ConnectionView | null> => {
    setBusy(andConnect ? "connect" : "save");
    setError(null);
    try {
      return await api.saveConnection(c, secrets);
    } catch (e) {
      setError(e as AppError);
      return null;
    } finally {
      setBusy(null);
    }
  };

  return {
    c,
    view,
    secrets,
    keyInfo,
    hasKey,
    busy,
    test,
    error,
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
