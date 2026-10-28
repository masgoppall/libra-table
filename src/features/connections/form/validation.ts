// What the form needs before saving, checked as the backend does (connections::validate),
// so missing fields can be marked in red instead of failing on the server.

import { DRIVER_LABEL, isFileDriver } from "@/domain/connection";
import { type Connection, type DriverEntry } from "@/types";

/** Mirrors db::data::MAX_PAGE_SIZE. */
export const MAX_PAGE_SIZE = 5000;

/** A field of the form: a catalog key ("host", "extra.account"…) or one of the fixed ones. */
export type FieldKey = string;

export interface FieldIssue {
  field: FieldKey;
  message: string;
}

/** The text a field shows ("" when unset). */
export function fieldValue(c: Connection, key: string): string {
  switch (key) {
    case "host":
      return c.db.host;
    case "port":
      return c.db.port === null ? "" : String(c.db.port);
    case "user":
      return c.db.user;
    case "database":
      return c.db.database;
    case "filePath":
      return c.db.filePath;
    default:
      return c.db.extra[key.replace(/^extra\./, "")] ?? "";
  }
}

/** Fields that must be filled in, given the current settings. */
export function requiredFields(c: Connection, entry: DriverEntry | undefined): Set<FieldKey> {
  const keys = new Set<FieldKey>(["name"]);
  if (isFileDriver(c.driver)) keys.add("filePath");
  for (const f of entry?.fields ?? []) if (f.required) keys.add(f.key);
  if (!isFileDriver(c.driver) && c.db.useSocket) keys.add("socket");
  if (c.ssh.enabled) keys.add("ssh.host");
  return keys;
}

export function validateConnection(c: Connection, entry: DriverEntry | undefined): FieldIssue[] {
  const issues: FieldIssue[] = [];
  const add = (field: FieldKey, message: string) => issues.push({ field, message });
  if (!c.name.trim()) add("name", "Give the connection a name.");
  if (isFileDriver(c.driver) && !c.db.filePath.trim()) {
    add("filePath", `Choose the ${DRIVER_LABEL[c.driver]} database file.`);
  }
  for (const f of entry?.fields ?? []) {
    if (!f.required || issues.some((i) => i.field === f.key)) continue;
    if (!fieldValue(c, f.key).trim()) add(f.key, `Enter the ${f.label.toLowerCase()}.`);
  }
  if (!isFileDriver(c.driver) && c.db.useSocket && !c.db.socket.trim()) {
    add("socket", "Enter the socket path, or switch back to host and port.");
  }
  if (c.ssh.enabled && !c.ssh.host.trim()) add("ssh.host", "Enter the SSH server address.");
  if (c.options.pageSize < 1 || c.options.pageSize > MAX_PAGE_SIZE) {
    add("pageSize", `Page size must be between 1 and ${MAX_PAGE_SIZE}.`);
  }
  return issues;
}
