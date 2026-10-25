// Sending key edits: safe mode may ask first (production), deletes always confirm, and
// errors (read-only connection, changed key…) become toasts.

import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { withSafeMode, writeDialogOptions } from "@/services/safeMode";
import { store } from "@/store";
import { type Db } from "@/types";
import { redisApi } from "./api";
import { type KeyEdit, type RedisValue } from "./types";

export interface KeyTarget {
  connectionId: string;
  db: Db;
}

/** Text of a key, field or member; binary values as hex. */
export function valueText(v: RedisValue): string {
  if (v === null) return "";
  if (typeof v === "object") return `0x${v.$binary}`;
  return String(v);
}

export function isBinary(v: RedisValue): boolean {
  return v !== null && typeof v === "object";
}

/** "No expiry", "45 s", "12 min", "3 h", "2 d". */
export function ttlText(ttl: number): string {
  if (ttl < 0) return "No expiry";
  if (ttl < 120) return `${ttl} s`;
  if (ttl < 7200) return `${Math.round(ttl / 60)} min`;
  if (ttl < 172800) return `${Math.round(ttl / 3600)} h`;
  return `${Math.round(ttl / 86400)} d`;
}

/** True when the edit ran. */
export async function runEdit(t: KeyTarget, edit: KeyEdit, success?: string): Promise<boolean> {
  const name = store.connection(t.connectionId)?.name ?? "";
  try {
    const send = (confirmed: boolean) => redisApi.edit(t.connectionId, t.db, edit, confirmed);
    const r = await withSafeMode(name, send);
    if (!r) return false;
    if (success) toastSuccess(success);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

/** Asks, then deletes `keys`. True when deleted. */
export async function deleteKeys(t: KeyTarget, keys: RedisValue[]): Promise<boolean> {
  if (!keys.length) return false;
  const guard = writeDialogOptions(t.connectionId);
  const r = await store.get().dialog({
    kind: "confirm",
    title: keys.length === 1 ? "Delete key" : `Delete ${keys.length} keys`,
    message: `${plural(keys.length, "key")} will be deleted with DEL.`,
    details: keys.slice(0, 20).map(valueText),
    okLabel: "Delete",
    danger: true,
    typeToConfirm: guard.typeToConfirm,
  });
  if (!r.ok) return false;
  try {
    // The dialog above is the confirmation safe mode asks for.
    await redisApi.edit(t.connectionId, t.db, { op: "delete", keys }, true);
    toastSuccess(`Deleted ${plural(keys.length, "key")}.`);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}
