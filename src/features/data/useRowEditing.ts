// Adding, deleting, duplicating (⌘D) and pasting (⌘V) rows in a data tab. Pasted rows
// that fit the table become new rows; a single value fills the focused cell (or that
// column of the selected rows); any other block fills the cells from the focused one,
// like a spreadsheet.

import { type CellPos, type CellRange } from "@/components/grid/types";
import { readText } from "@/lib/clipboard";
import { plural } from "@/lib/format";
import { toastInfo } from "@/services/errors";
import { type CellInput, type GridValue, type PageResult } from "@/types";
import * as edits from "./pending";
import { type Pending } from "./pending";
import { fieldValue, type NewRow, pastedFields, pastedRows, rowFromCells } from "./rowCopies";

export interface RowEditingContext {
  result: PageResult | null;
  pending: Pending;
  update: (fn: (p: Pending) => Pending) => void;
  getCell: (r: number, c: number) => GridValue;
  /** Column indexes as the grid shows them (order, hidden columns left out). */
  visibleColumns: number[];
  selected: Set<number>;
  select: (rows: Set<number>, focus: number | null) => void;
  readOnlyConnection: boolean;
  /** The driver can edit rows at all. */
  editRows: boolean;
  /** Rows of this table can be edited (it has a primary key, the connection allows it). */
  editable: boolean;
}

const NO_PK_HINT = "Add a primary key in the structure tab, or insert rows with a query.";
const READ_ONLY_HINT = "Turn off Read-only in the connection settings to edit rows.";

const range = (from: number, count: number) =>
  new Set(Array.from({ length: count }, (_, i) => from + i));

export function useRowEditing(x: RowEditingContext) {
  const baseRows = x.result?.rows.length ?? 0;
  const rowCount = () => baseRows + x.pending.inserts.length;
  const isDeleted = (r: number) => x.pending.deletes.has(r);

  /** False, after saying why, when rows can't be added or changed here. */
  const canEdit = (): boolean => {
    if (x.readOnlyConnection) toastInfo("This connection is read-only.", READ_ONLY_HINT);
    else if (!x.editRows) toastInfo("Editing rows isn't available for this database.");
    else if (!x.editable)
      toastInfo("This table has no primary key, so it is read-only here.", NO_PK_HINT);
    return x.editable;
  };

  const addRow = () => {
    if (!canEdit()) return;
    const r = rowCount();
    x.update(edits.addInsert);
    x.select(new Set([r]), r);
  };

  /** New rows are dropped; loaded rows toggle their "deleted" mark. */
  const deleteSelected = () => {
    if (!x.editable || x.selected.size === 0) return;
    x.update((p) => edits.toggleDeletes(p, x.selected, baseRows));
    x.select(new Set(), null);
  };

  const addRows = (made: NewRow[]) => {
    const first = rowCount();
    x.update((p) =>
      edits.addInserts(
        p,
        made.map((m) => m.values),
      ),
    );
    x.select(range(first, made.length), first);
    const skipped = [...new Set(made.flatMap((m) => m.skipped))];
    if (skipped.length) {
      toastInfo(
        `${plural(made.length, "row")} added; binary values weren't copied.`,
        `${skipped.join(", ")} will take the column default.`,
      );
    }
  };

  const duplicate = () => {
    const { result } = x;
    if (!result || !canEdit()) return;
    const rows = [...x.selected].sort((a, b) => a - b).filter((r) => !isDeleted(r));
    if (!rows.length) return toastInfo("Select the rows to duplicate.");
    const cols = result.columns.map((_, c) => c);
    addRows(
      rows.map((r) =>
        rowFromCells(
          result.columns,
          cols.map((c) => x.getCell(r, c)),
        ),
      ),
    );
  };

  /** Sets cells in one undoable step. */
  const setCells = (cells: { row: number; col: number; value: CellInput }[]) => {
    const { result } = x;
    if (!result || !cells.length) return;
    x.update((p) =>
      cells.reduce((acc, c) => edits.editCell(acc, result, c.row, c.col, c.value), p),
    );
  };

  /** One value: every cell of the selected block, the focused cell, or that column of
   * every selected row. */
  const pasteValue = (at: CellPos, text: string, block: CellRange | null) => {
    const value = (col: number) => fieldValue(x.result?.columns[col], text);
    if (block) {
      const cells = [];
      for (let row = block.top; row <= block.bottom; row++) {
        if (!isDeleted(row))
          cells.push(...block.cols.map((col) => ({ row, col, value: value(col) })));
      }
      return setCells(cells);
    }
    const rows = x.selected.has(at.row) && x.selected.size > 1 ? [...x.selected] : [at.row];
    setCells(
      rows.filter((r) => !isDeleted(r)).map((row) => ({ row, col: at.col, value: value(at.col) })),
    );
  };

  /** A block of fields into the cells from `at` on; what falls outside is skipped. */
  const pasteBlock = (at: CellPos, fields: (string | null)[][]) => {
    const start = x.visibleColumns.indexOf(at.col);
    const cells: { row: number; col: number; value: CellInput }[] = [];
    let outside = 0;
    fields.forEach((line, i) =>
      line.forEach((text, j) => {
        const row = at.row + i;
        const col = x.visibleColumns[start + j];
        if (row >= rowCount() || col === undefined) outside += 1;
        else if (text !== null && !isDeleted(row)) {
          cells.push({ row, col, value: fieldValue(x.result?.columns[col], text) });
        }
      }),
    );
    setCells(cells);
    if (outside)
      toastInfo(`${plural(outside, "pasted value")} fell outside the table and were skipped.`);
  };

  const paste = async (at: CellPos | null, block: CellRange | null = null) => {
    const { result } = x;
    if (!result) return;
    const text = await readText();
    if (!text) return toastInfo("The clipboard is empty.");
    if (!canEdit()) return;
    const fields = pastedFields(text);
    const only = fields.length === 1 && fields[0]?.length === 1 ? fields[0][0] : undefined;
    if (at && only !== undefined) return only === null ? undefined : pasteValue(at, only, block);
    const rows = pastedRows(result.columns, text);
    if (rows) return addRows(rows.rows);
    const from = block ? { row: block.top, col: block.cols[0] ?? at?.col ?? 0 } : at;
    if (from) return pasteBlock(from, fields);
    toastInfo(
      "The copied rows don't match this table's columns.",
      "Select a cell to paste the values into the cells from there.",
    );
  };

  return { addRow, deleteSelected, duplicate, paste, setCells };
}
