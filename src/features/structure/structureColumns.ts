// The structure grid's columns as TablePlus shows them (column_name, data_type…), what
// each cell reads, and the popup menus of the cells that have one.

import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { TYPE_MENU, typeFromMenu } from "@/domain/ddl";
import { type ColumnSpec, type Dialect } from "@/types";
import {
  baseType,
  type ColRow,
  columnForeignKeys,
  type FkRow,
  isTextType,
  norm,
} from "./designerModel";

export type FieldKey =
  | "name"
  | "dataType"
  | "charset"
  | "collation"
  | "nullable"
  | "default"
  | "extra"
  | "foreignKey"
  | "comment";

export interface GridColumn {
  key: FieldKey;
  label: string;
  /** A popup menu (the ⌃⌄ at the right). */
  popup?: boolean;
  center?: boolean;
}

const COLUMNS: Record<FieldKey, GridColumn> = {
  name: { key: "name", label: "column_name" },
  dataType: { key: "dataType", label: "data_type", popup: true },
  charset: { key: "charset", label: "character_set" },
  collation: { key: "collation", label: "collation" },
  nullable: { key: "nullable", label: "is_nullable", popup: true, center: true },
  default: { key: "default", label: "column_default", popup: true },
  extra: { key: "extra", label: "extra", popup: true },
  foreignKey: { key: "foreignKey", label: "foreign_key" },
  comment: { key: "comment", label: "comment" },
};

const LAYOUT: Record<Dialect, FieldKey[]> = {
  mysql: [
    "name",
    "dataType",
    "charset",
    "collation",
    "nullable",
    "default",
    "extra",
    "foreignKey",
    "comment",
  ],
  postgres: ["name", "dataType", "nullable", "default", "extra", "foreignKey", "comment"],
  sqlite: ["name", "dataType", "nullable", "default", "extra", "foreignKey"],
};

export function gridColumns(driver: Dialect): GridColumn[] {
  return LAYOUT[driver].map((k) => COLUMNS[k]);
}

/** What a cell reads; `dim` for TablePlus' grey NULL / EMPTY. */
export interface CellText {
  text: string;
  dim?: boolean;
}

const NULL: CellText = { text: "NULL", dim: true };
const EMPTY: CellText = { text: "EMPTY", dim: true };

/** `'it''s'` → `it's`; other expressions as written. */
function defaultText(d: string | null): CellText {
  if (d === null || !norm(d)) return NULL;
  if (d === "''") return EMPTY;
  const literal = /^'((?:[^']|'')*)'$/.exec(d.trim());
  return { text: literal ? (literal[1] ?? "").replace(/''/g, "'") : d };
}

function extraText(r: ColRow): CellText {
  const c = r.cur;
  const parts = [c.autoIncrement ? "auto_increment" : "", norm(c.extra)].filter(Boolean);
  if (parts.length) return { text: parts.join(" ") };
  // MySQL marks expression defaults; shown while the default is the one loaded.
  const generated = /default_generated/i.test(r.info?.extra ?? "");
  if (r.orig && generated && norm(r.orig.default) === norm(c.default)) {
    return { text: "DEFAULT_GENERATED" };
  }
  return EMPTY;
}

function foreignKeyText(fks: FkRow[], column: string): CellText {
  const fk = columnForeignKeys(fks, column)[0];
  if (!fk) return EMPTY;
  const at = fk.cur.columns.indexOf(column);
  const target = fk.cur.columns.length > 1 ? fk.cur.refColumns.join(", ") : fk.cur.refColumns[at];
  return { text: `${fk.cur.refTable}(${target ?? ""})` };
}

export function cellText(r: ColRow, key: FieldKey, fks: FkRow[]): CellText {
  const c = r.cur;
  switch (key) {
    case "name":
      return { text: c.name };
    case "dataType":
      return { text: c.dataType };
    case "charset":
      return isTextType(c.dataType) && norm(c.charset) ? { text: norm(c.charset) } : NULL;
    case "collation":
      return isTextType(c.dataType) && norm(c.collation) ? { text: norm(c.collation) } : NULL;
    case "nullable":
      return { text: c.nullable ? "YES" : "NO" };
    case "default":
      return defaultText(c.default);
    case "extra":
      return extraText(r);
    case "foreignKey":
      return foreignKeyText(fks, c.name.trim());
    case "comment":
      return norm(c.comment) ? { text: norm(c.comment) } : EMPTY;
  }
}

/** The text an inline editor starts with. */
export function editText(c: ColumnSpec, key: FieldKey): string {
  switch (key) {
    case "default":
      return c.default ?? "";
    case "extra":
      return [c.autoIncrement ? "auto_increment" : "", norm(c.extra)].filter(Boolean).join(" ");
    case "nullable":
    case "foreignKey":
      return "";
    default:
      return (c[key] as string | null) ?? "";
  }
}

/** What typing `text` into a cell changes. */
export function editPatch(key: FieldKey, text: string): Partial<ColumnSpec> {
  const v = text.trim();
  switch (key) {
    case "name":
      return { name: text };
    case "dataType":
      return { dataType: v };
    case "charset":
    case "collation":
      return { [key]: v && v.toUpperCase() !== "NULL" ? v : null };
    case "default":
      return { default: v && v.toUpperCase() !== "NULL" ? v : null };
    case "extra": {
      const words = v.split(/\s+/).filter(Boolean);
      const autoIncrement = words.some((w) => w.toLowerCase() === "auto_increment");
      const rest = words.filter((w) => w.toLowerCase() !== "auto_increment").join(" ");
      return { autoIncrement, extra: rest || null };
    }
    case "comment":
      return { comment: text.trim() ? text : null };
    default:
      return {};
  }
}

// ------------------------------------------------------------------ popup menus

export interface PopupActions {
  set: (patch: Partial<ColumnSpec>) => void;
  /** "Manual input…": edit the cell as text. */
  manual: () => void;
}

const MANUAL = "Manual input…";

export function typeMenu(driver: Dialect, c: ColumnSpec, a: PopupActions): MenuItem[] {
  const current = baseType(c.dataType);
  const items: MenuItem[] = TYPE_MENU[driver].map((t) => ({
    label: t,
    selected: t.toLowerCase() === current,
    // Picking the same type keeps its length ("varchar(100)").
    onClick: () => t.toLowerCase() !== current && a.set({ dataType: typeFromMenu(t) }),
  }));
  return [...items, SEPARATOR, { label: MANUAL, onClick: a.manual }];
}

export function nullableMenu(c: ColumnSpec, a: PopupActions): MenuItem[] {
  return [
    { label: "YES", selected: c.nullable, onClick: () => a.set({ nullable: true }) },
    { label: "NO", selected: !c.nullable, onClick: () => a.set({ nullable: false }) },
  ];
}

const TIME_TYPES = ["timestamp", "datetime", "date", "time", "timestamptz"];

export function defaultMenu(c: ColumnSpec, a: PopupActions): MenuItem[] {
  const d = norm(c.default);
  const items: MenuItem[] = [
    { label: "NULL", selected: !d, onClick: () => a.set({ default: null }) },
    { label: "EMPTY", selected: d === "''", onClick: () => a.set({ default: "''" }) },
  ];
  if (TIME_TYPES.includes(baseType(c.dataType))) {
    const now = "CURRENT_TIMESTAMP";
    items.push(SEPARATOR, {
      label: now,
      selected: d.toUpperCase() === now,
      onClick: () => a.set({ default: now }),
    });
  }
  return [...items, SEPARATOR, { label: MANUAL, onClick: a.manual }];
}

const SERIAL = "SERIAL DEFAULT VALUE";
const ON_UPDATE = "on update CURRENT_TIMESTAMP";

export function extraMenu(driver: Dialect, c: ColumnSpec, a: PopupActions): MenuItem[] {
  const extra = norm(c.extra).toLowerCase();
  const none = !c.autoIncrement && !extra;
  const items: MenuItem[] = [
    {
      label: "NONE",
      selected: none,
      onClick: () => a.set({ autoIncrement: false, extra: null }),
    },
    SEPARATOR,
    {
      label: "AUTO_INCREMENT",
      selected: c.autoIncrement && !extra,
      onClick: () => a.set({ autoIncrement: true, extra: null }),
    },
  ];
  if (driver !== "mysql") return items;
  return [
    ...items,
    {
      label: SERIAL,
      selected: !c.autoIncrement && extra === SERIAL.toLowerCase(),
      onClick: () => a.set({ autoIncrement: false, extra: SERIAL }),
    },
    {
      label: ON_UPDATE.toUpperCase(),
      selected: !c.autoIncrement && extra === ON_UPDATE.toLowerCase(),
      onClick: () => a.set({ autoIncrement: false, extra: ON_UPDATE }),
    },
    SEPARATOR,
    { label: MANUAL, onClick: a.manual },
  ];
}
