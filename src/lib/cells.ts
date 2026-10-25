// Cell values: what they are, and how they read in the grid and the detail panel.

import { type BinaryCell, type Cell, type DefaultMarker, type GridValue } from "@/types";
import { formatBytes } from "./format";

export const DEFAULT: DefaultMarker = { $default: true };

export function isBinary(v: GridValue): v is BinaryCell {
  return typeof v === "object" && v !== null && "$binary" in v;
}

export function isDefaultMarker(v: unknown): v is DefaultMarker {
  return typeof v === "object" && v !== null && "$default" in v;
}

/** The cell value, with a pending DEFAULT read as NULL. */
export function cellValue(v: GridValue): Cell {
  return isDefaultMarker(v) ? null : v;
}

/** True for NULL and for a pending DEFAULT: both edit as an empty field. */
export function isEmptyCell(v: GridValue): boolean {
  return v === null || isDefaultMarker(v);
}

const MAX_DISPLAY = 500;

/** One-line text for a grid cell (long values are truncated by CSS). */
export function displayCell(v: Cell): string {
  if (v === null) return "NULL";
  if (isBinary(v)) return `<binary ${formatBytes(v.size)}>`;
  if (typeof v !== "string") return String(v);
  if (v.length > MAX_DISPLAY) return v.slice(0, MAX_DISPLAY) + "…";
  return v.replace(/\r?\n/g, " ⏎ ");
}

/** Full text of a cell, for editing, the detail panel and the clipboard. */
export function cellText(v: GridValue): string {
  if (isEmptyCell(v)) return "";
  if (isBinary(v)) return "0x" + v.$binary;
  return String(v);
}

/** Whether an edit leaves the value as it was (compared as text, like the user sees it). */
export function sameValue(original: Cell, value: unknown): boolean {
  if (original === value) return true;
  if (original === null || isBinary(original)) return false;
  if (value === null || typeof value === "object") return false;
  return String(original) === String(value);
}

export function isNumericType(typeName: string): boolean {
  return (
    /int|serial|numeric|decimal|real|double|float|money|year/i.test(typeName) &&
    !/interval|point/i.test(typeName)
  );
}

export function prettyJson(text: string): string | null {
  const t = text.trim();
  if (!(t.startsWith("{") || t.startsWith("["))) return null;
  try {
    return JSON.stringify(JSON.parse(t), null, 2);
  } catch {
    return null;
  }
}
