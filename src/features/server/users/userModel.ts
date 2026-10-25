// Pure rules of the users tab: role form ↔ RoleSpec, grantees, grant targets and the
// privileges each target accepts.

import {
  type GrantTarget,
  type Grantee,
  type MysqlAccount,
  type PgRole,
  type PrivilegeOption,
  type RoleSpec,
  type TargetKind,
} from "@/types";

export const TARGET_LABELS: Record<TargetKind, string> = {
  global: "Server (*.*)",
  database: "Database",
  schema: "Schema",
  all_tables: "All tables in schema",
  table: "Table",
};

export function emptyRole(): RoleSpec {
  return {
    name: "",
    login: true,
    superuser: false,
    createDb: false,
    createRole: false,
    replication: false,
    connLimit: null,
    validUntil: null,
    memberOf: [],
  };
}

/** The form values of an existing role ("infinity" and -1 mean no limit). */
export function roleToSpec(r: PgRole): RoleSpec {
  return {
    name: r.name,
    login: r.login,
    superuser: r.superuser,
    createDb: r.createDb,
    createRole: r.createRole,
    replication: r.replication,
    connLimit: r.connLimit < 0 ? null : r.connLimit,
    validUntil: r.validUntil && r.validUntil !== "infinity" ? r.validUntil : null,
    memberOf: [...r.memberOf],
  };
}

/** "" → no limit; a whole number ≥ 0 → that limit; anything else → undefined (invalid). */
export function parseConnLimit(text: string): number | null | undefined {
  const t = text.trim();
  if (!t) return null;
  return /^\d+$/.test(t) ? Number(t) : undefined;
}

export const accountGrantee = (a: MysqlAccount): Grantee => ({ name: a.user, host: a.host });
export const roleGrantee = (name: string): Grantee => ({ name, host: null });

export function granteeLabel(g: Grantee): string {
  return g.host ? `${g.name}@${g.host}` : g.name;
}

export function privilegesFor(options: PrivilegeOption[], kind: TargetKind): string[] {
  return options.find((o) => o.target === kind)?.privileges ?? [];
}

/** What the grant dialog has picked for each kind of target. */
export interface TargetFields {
  database: string;
  schema: string;
  table: string;
}

/** The target, or null while a name it needs is missing. */
export function buildTarget(kind: TargetKind, f: TargetFields): GrantTarget | null {
  switch (kind) {
    case "global":
      return { on: "global" };
    case "database":
      return f.database ? { on: "database", name: f.database } : null;
    case "schema":
      return f.schema ? { on: "schema", name: f.schema } : null;
    case "all_tables":
      return f.schema ? { on: "all_tables", schema: f.schema } : null;
    case "table":
      return f.schema && f.table ? { on: "table", schema: f.schema, name: f.table } : null;
  }
}

/** "ALL PRIVILEGES" stands alone: picking it clears the others and the other way round. */
export function togglePrivilege(picked: string[], p: string, on: boolean): string[] {
  const ALL = "ALL PRIVILEGES";
  if (!on) return picked.filter((x) => x !== p);
  if (p === ALL) return [ALL];
  return [...picked.filter((x) => x !== ALL && x !== p), p];
}

/** Role attributes as short labels for the table ("Login", "Superuser"…). */
export function roleFlags(r: PgRole): string[] {
  const flags: [boolean, string][] = [
    [r.superuser, "Superuser"],
    [r.createDb, "Create DB"],
    [r.createRole, "Create role"],
    [r.replication, "Replication"],
  ];
  return flags.filter(([on]) => on).map(([, label]) => label);
}

export function matchesSearch(text: string, ...fields: string[]): boolean {
  const needle = text.trim().toLowerCase();
  return !needle || fields.some((f) => f.toLowerCase().includes(needle));
}
