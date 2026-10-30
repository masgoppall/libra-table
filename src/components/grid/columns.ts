// Column geometry: default widths by type, user order, horizontal positions.

import { cellValue, displayCell, isNumericType } from "@/lib/cells";
import { type GridValue } from "@/types";
import { type GridColumn } from "./types";

export const ROW_H = 24;
export const ROWNUM_W = 52;
export const MIN_W = 48;
const DEFAULT_W = 160;
const MAX_DEFAULT_W = 320;

function widthByType(typeName: string): number {
  if (isNumericType(typeName)) return 110;
  if (/bool/i.test(typeName)) return 80;
  if (/date|time/i.test(typeName)) return 180;
  return DEFAULT_W;
}

export function defaultWidth(c: GridColumn): number {
  // Wide enough for the header (name + type), within reason.
  const header = 28 + c.name.length * 7.5 + c.typeName.length * 6 + (c.isPk ? 16 : 0);
  return Math.round(Math.min(MAX_DEFAULT_W, Math.max(widthByType(c.typeName), header)));
}

/** Column indexes in display order: the saved order first, new columns after; hidden
 * columns left out. */
export function orderColumns(
  columns: GridColumn[],
  order: string[] | undefined,
  hidden: string[] = [],
): number[] {
  const all = columns.map((_, i) => i);
  const saved = (order ?? [])
    .map((n) => columns.findIndex((c) => c.name === n))
    .filter((i) => i >= 0);
  const ordered = [...saved, ...all.filter((i) => !saved.includes(i))];
  return ordered.filter((i) => !hidden.includes(columns[i]?.name ?? ""));
}

/** The names in display order after dropping `from` before `target`. */
export function reorder(names: string[], from: string, target: string): string[] {
  const rest = names.filter((n) => n !== from);
  rest.splice(rest.indexOf(target), 0, from);
  return rest;
}

/** Left edge of a column inside the scroll area (after the row numbers, `numW` wide). */
export function columnLeft(
  ordered: number[],
  col: number,
  width: (i: number) => number,
  numW = ROWNUM_W,
): number {
  let left = numW;
  for (const i of ordered) {
    if (i === col) break;
    left += width(i);
  }
  return left;
}

const FIT_SAMPLE = 500;
const FIT_MAX = 600;
let canvas: HTMLCanvasElement | null = null;

/** Width of `text` in `font`; a rough estimate where there's no canvas (tests). */
function textWidth(text: string, font: string): number {
  canvas ??= document.createElement("canvas");
  const ctx = canvas.getContext?.("2d");
  if (!ctx) return text.length * 7;
  ctx.font = font;
  return ctx.measureText(text).width;
}

/** A width that shows the header and the longest of the first rows (TablePlus' "Expand
 * columns to fit the content"), within reason. */
export function fitWidth(
  c: GridColumn,
  values: (row: number) => GridValue,
  rowCount: number,
  font: string,
): number {
  let widest = textWidth(c.name, `600 ${font}`) + textWidth(c.typeName, font) * 0.8 + 34;
  for (let r = 0; r < Math.min(rowCount, FIT_SAMPLE); r++) {
    const text = displayCell(cellValue(values(r)));
    widest = Math.max(widest, textWidth(text.slice(0, 200), font) + 18);
  }
  return Math.round(Math.max(MIN_W, Math.min(FIT_MAX, widest)));
}
