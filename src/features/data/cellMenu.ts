// Right-click menu of a grid cell: copy in several formats, set values, filter, delete.

import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { cellText, cellValue, DEFAULT } from "@/lib/cells";
import { type CopyFormat } from "@/lib/copyRows";
import { copyText } from "@/lib/clipboard";
import { type Cell, type CellInput, type GridValue } from "@/types";

export interface CellMenuContext extends CopyMenuContext {
  editable: boolean;
  markedDeleted: boolean;
  setValue: (v: CellInput) => void;
  filterBy: (v: Cell) => void;
  deleteRows: () => void;
  duplicate: () => void;
  paste: () => void;
  /** Absent when the driver can't load whole values. */
  quickLook?: () => void;
  /** Set when the column is a foreign key and the cell has a value. */
  reference?: { table: string; open: () => void };
}

const COPY_FORMATS: [CopyFormat, string][] = [
  ["tsv", "TSV (tab separated)"],
  ["csv", "CSV"],
  ["json", "JSON"],
  ["sql", "SQL INSERT"],
  ["markdown", "Markdown"],
  ["html", "HTML"],
];

/** The "Copy as ▸" submenu. */
export const copyAsMenu = (copy: (format: CopyFormat) => void): MenuItem[] =>
  COPY_FORMATS.map(([format, label]) => ({ label, onClick: () => copy(format) }));

export interface CopyMenuContext {
  value: GridValue;
  rowCount: number;
  cellCount: number;
  copy: (format: CopyFormat) => void;
  copyCells: (format: CopyFormat) => void;
}

/** Copy items shared by data tabs and query results. */
export function copyMenu(c: CopyMenuContext): MenuItem[] {
  const rows = c.rowCount > 1 ? `${c.rowCount} rows` : "row";
  const block: MenuItem[] = c.cellCount
    ? [
        { label: `Copy ${c.cellCount} cells`, hint: "⌘C", onClick: () => c.copyCells("tsv") },
        { label: "Copy cells as", submenu: copyAsMenu(c.copyCells) },
      ]
    : [];
  return [
    ...block,
    { label: "Copy cell", onClick: () => void copyText(cellText(c.value)) },
    {
      label: `Copy ${rows}`,
      hint: c.cellCount ? undefined : "⌘C",
      onClick: () => c.copy("tsv"),
    },
    { label: c.rowCount > 1 ? "Copy rows as" : "Copy row as", submenu: copyAsMenu(c.copy) },
  ];
}

function deleteLabel(c: CellMenuContext): string {
  if (c.markedDeleted) return "Undo delete";
  return c.rowCount > 1 ? `Delete ${c.rowCount} rows` : "Delete row";
}

export function cellMenu(c: CellMenuContext): MenuItem[] {
  const ro = !c.editable;
  const follow: MenuItem[] = c.reference
    ? [
        { label: `Open referenced row in ${c.reference.table}`, onClick: c.reference.open },
        SEPARATOR,
      ]
    : [];
  return [
    ...follow,
    ...(c.quickLook ? [{ label: "Quick Look", hint: "Space", onClick: c.quickLook }] : []),
    ...copyMenu(c),
    { label: "Paste", hint: "⌘V", disabled: ro, onClick: c.paste },
    SEPARATOR,
    {
      label: c.rowCount > 1 ? `Duplicate ${c.rowCount} rows` : "Duplicate row",
      hint: "⌘D",
      disabled: ro,
      onClick: c.duplicate,
    },
    { label: "Set NULL", disabled: ro, onClick: () => c.setValue(null) },
    { label: "Set empty string", disabled: ro, onClick: () => c.setValue("") },
    { label: "Set DEFAULT", disabled: ro, onClick: () => c.setValue(DEFAULT) },
    { label: "Filter by this value", onClick: () => c.filterBy(cellValue(c.value)) },
    SEPARATOR,
    {
      label: deleteLabel(c),
      hint: "⌫",
      danger: !c.markedDeleted,
      disabled: ro,
      onClick: c.deleteRows,
    },
  ];
}
