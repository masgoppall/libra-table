// Query variables: `:name` placeholders in the editor, filled in before a run.
// A `:` only starts a variable outside strings, quoted names, comments and Postgres
// dollar-quoted bodies, and never as part of `::type`, `:=` or an array slice `a[1:n]`.
// Values become SQL literals (quoted like the rest of the app); see `renderValue`.

import { type Dialect, type QueryVariable, type VariableFormat } from "@/types";
import { sqlLiteral } from "./sql";

export type { VariableFormat } from "@/types";

export interface VariableOptions {
  format?: VariableFormat;
  /** Variables inside quoted strings count too (`'%:term%'`). */
  inStrings?: boolean;
}

const PATTERNS: Record<VariableFormat, RegExp> = {
  colon: /^:([A-Za-z_][A-Za-z0-9_]*)/,
  dollar: /^\$([A-Za-z_][A-Za-z0-9_]*)/,
  "dollar-braces": /^\$\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}/,
  "double-braces": /^\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/,
  percent: /^%([A-Za-z_][A-Za-z0-9_]*)%/,
};

export const VARIABLE_FORMATS: [VariableFormat, string][] = [
  ["colon", "Colon :name"],
  ["dollar", "Dollar $name"],
  ["dollar-braces", "Dollar braces ${name}"],
  ["double-braces", "Double braces {{name}}"],
  ["percent", "Percent %name%"],
];

export interface VariableUse {
  name: string;
  /** Offset of the `:`. */
  from: number;
  /** Offset just past the name. */
  to: number;
}

const NAME_START = /[A-Za-z_]/;
const NAME_CHAR = /[A-Za-z0-9_]/;
/** `auto` treats canonical numbers (no leading zeros) as numbers, anything else as text. */
const AUTO_NUMBER = /^-?(0|[1-9]\d*)(\.\d+)?$/;
const NUMBER = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;

const isName = (c: string | undefined) => !!c && NAME_CHAR.test(c);

/** End of a quoted run starting at `i` (doubled quotes; backslashes when `bs`). */
function skipQuoted(sql: string, i: number, bs: boolean): number {
  const q = sql[i];
  let j = i + 1;
  while (j < sql.length) {
    const c = sql[j];
    if (bs && c === "\\") j += 2;
    else if (c === q && sql[j + 1] === q) j += 2;
    else if (c === q) return j + 1;
    else j += 1;
  }
  return j;
}

function skipBlockComment(sql: string, i: number, nested: boolean): number {
  let depth = 0;
  let j = i;
  while (j < sql.length) {
    if (sql.startsWith("/*", j)) {
      depth = nested ? depth + 1 : 1;
      j += 2;
    } else if (sql.startsWith("*/", j)) {
      depth -= 1;
      j += 2;
      if (depth <= 0) return j;
    } else j += 1;
  }
  return j;
}

/** A Postgres `$tag$ … $tag$` body at `i`, or null (`$1` is a parameter). */
function skipDollar(sql: string, i: number): number | null {
  if (isName(sql[i - 1])) return null;
  const m = /^\$([A-Za-z_][A-Za-z0-9_]*)?\$/.exec(sql.slice(i));
  if (!m) return null;
  const end = sql.indexOf(m[0], i + m[0].length);
  return end < 0 ? sql.length : end + m[0].length;
}

function isLineComment(sql: string, i: number, driver: Dialect): boolean {
  if (driver === "mysql" && sql[i] === "#") return true;
  if (!sql.startsWith("--", i)) return false;
  // MySQL needs whitespace (or the end) after `--`.
  return driver !== "mysql" || i + 2 >= sql.length || /\s/.test(sql[i + 2] ?? "");
}

/** Skips whatever starts at `i` that can't hold a variable; null if nothing does. */
function skipNonCode(sql: string, i: number, driver: Dialect): number | null {
  const c = sql[i];
  if (c === "'") {
    // Postgres E'…' strings take backslash escapes (but `LIKE'…'` is not one).
    const eString = /[Ee]/.test(sql[i - 1] ?? "") && !isName(sql[i - 2]);
    return skipQuoted(sql, i, driver === "mysql" || (driver === "postgres" && eString));
  }
  if (c === '"' || c === "`") return skipQuoted(sql, i, driver === "mysql" && c === '"');
  if (isLineComment(sql, i, driver)) {
    const nl = sql.indexOf("\n", i);
    return nl < 0 ? sql.length : nl;
  }
  if (sql.startsWith("/*", i)) return skipBlockComment(sql, i, driver === "postgres");
  if (c === "$" && driver === "postgres") return skipDollar(sql, i);
  return null;
}

/** A variable written as `format` at `i`, or null. */
function variableAt(sql: string, i: number, format: VariableFormat): VariableUse | null {
  if (format === "colon" && sql[i + 1] === ":") return null;
  if ((format === "colon" || format === "dollar") && isName(sql[i - 1])) return null;
  const m = PATTERNS[format].exec(sql.slice(i, i + 200));
  return m?.[1] ? { name: m[1], from: i, to: i + m[0].length } : null;
}

/** Quoted strings are searched too with `inStrings`; comments never are. */
function skipFor(sql: string, i: number, driver: Dialect, o: VariableOptions): number | null {
  if (o.inStrings && sql[i] === "'") return null;
  // `$name` and `${name}` aren't Postgres dollar quotes.
  if (sql[i] === "$" && o.format !== undefined && o.format !== "colon") {
    if (variableAt(sql, i, o.format)) return null;
  }
  return skipNonCode(sql, i, driver);
}

/** Every variable in `sql`, in order. */
export function findVariables(
  sql: string,
  driver: Dialect,
  o: VariableOptions = {},
): VariableUse[] {
  const format = o.format ?? "colon";
  const uses: VariableUse[] = [];
  let i = 0;
  while (i < sql.length) {
    const skipped = skipFor(sql, i, driver, o);
    if (skipped !== null) {
      i = skipped;
      continue;
    }
    if (format === "colon" && sql[i] === ":" && sql[i + 1] === ":") {
      i += 2;
      continue;
    }
    const use =
      NAME_START.test(sql[i + 1] ?? "") || format.endsWith("braces")
        ? variableAt(sql, i, format)
        : null;
    if (use) {
      uses.push(use);
      i = use.to;
      continue;
    }
    i += 1;
  }
  return uses;
}

/** Variable names in order of first use. */
export function variableNames(sql: string, driver: Dialect, o: VariableOptions = {}): string[] {
  return [...new Set(findVariables(sql, driver, o).map((u) => u.name))];
}

/** The SQL for one value. Throws (with a message for the user) if it isn't valid. */
export function renderValue(driver: Dialect, v: QueryVariable): string {
  const value = v.value.trim();
  switch (v.mode) {
    case "null":
      return "NULL";
    case "raw":
      if (!value) throw new Error(`:${v.name} needs SQL.`);
      return v.value;
    case "number":
      if (!NUMBER.test(value)) throw new Error(`:${v.name} is not a number.`);
      return value;
    case "text":
      return sqlLiteral(driver, v.value);
    case "auto":
      return AUTO_NUMBER.test(value) ? value : sqlLiteral(driver, v.value);
  }
}

/** `sql` with every variable replaced by its value. Throws if one has no value. */
export function substituteVariables(
  sql: string,
  driver: Dialect,
  values: QueryVariable[],
  o: VariableOptions = {},
): string {
  const byName = new Map(values.map((v) => [v.name, v]));
  let out = sql;
  for (const use of findVariables(sql, driver, o).reverse()) {
    const v = byName.get(use.name);
    if (!v) throw new Error(`No value for :${use.name}.`);
    out = out.slice(0, use.from) + renderValue(driver, v) + out.slice(use.to);
  }
  return out;
}

/** Remembered values for `names`, new ones empty ("auto"). */
export function variablesFor(names: string[], remembered: QueryVariable[] = []): QueryVariable[] {
  return names.map(
    (name) => remembered.find((v) => v.name === name) ?? { name, value: "", mode: "auto" },
  );
}

/** Keeps the latest values first and forgets nothing still in use elsewhere. */
export function rememberVariables(
  previous: QueryVariable[] = [],
  latest: QueryVariable[],
): QueryVariable[] {
  const names = new Set(latest.map((v) => v.name));
  return [...latest, ...previous.filter((v) => !names.has(v.name))].slice(0, 50);
}
