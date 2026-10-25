// Right-click menu of a grid cell: copy in several formats, set values, filter, delete.

import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { cellText, cellValue, DEFAULT } from "@/lib/cells";
import { type CopyFormat } from "@/lib/copyRows";
import { copyText } from "@/lib/clipboard";
import { type Cell, type CellInput, type GridValue } from "@/types";

export interface CellMenuContext {
  value: GridValue;
  /** Rows the copy and delete items act on. */
  rowCount: number;
  editable: boolean;
  markedDeleted: boolean;
  copy: (format: CopyFormat) => void;
  setValue: (v: CellInput) => void;
  filterBy: (v: Cell) => void;
  deleteRows: () => void;
  quickLook: () => void;
  /** Set when the column is a foreign key and the cell has a value. */
  reference?: { table: string; open: () => void };
}

const COPY_FORMATS: [CopyFormat, string][] = [
  ["csv", "Copy as CSV"],
  ["json", "Copy as JSON"],
  ["sql", "Copy as SQL INSERT"],
  ["markdown", "Copy as Markdown"],
];

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
    { label: "Quick Look", hint: "Space", onClick: c.quickLook },
    { label: "Copy cell", onClick: () => void copyText(cellText(c.value)) },
    {
      label: c.rowCount > 1 ? `Copy ${c.rowCount} rows` : "Copy row",
      hint: "⌘C",
      onClick: () => c.copy("tsv"),
    },
    ...COPY_FORMATS.map(([format, label]) => ({ label, onClick: () => c.copy(format) })),
    SEPARATOR,
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
