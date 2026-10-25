// ER diagram geometry without UI: table box sizes, the automatic layout (layers by
// foreign key dependencies, a grid when there are none), connector paths, zoom and fit.

import { type DiagramLayout, type DiagramTable, type Point } from "@/types";

export const HEADER_H = 30;
export const ROW_H = 20;
const PAD_BOTTOM = 6;
const CHAR_W = 7;
const MIN_W = 170;
const MAX_W = 340;
const GAP_X = 90;
const GAP_Y = 36;
/** A layer taller than this many tables is split into several columns. */
const MAX_PER_COLUMN = 6;
export const MIN_ZOOM = 0.15;
export const MAX_ZOOM = 2.5;

export interface Size {
  w: number;
  h: number;
}

export interface Rect extends Point, Size {}

export interface View {
  x: number;
  y: number;
  scale: number;
}

export function tableSize(t: DiagramTable): Size {
  const longest = Math.max(
    t.name.length + 4,
    ...t.columns.map((c) => c.name.length + c.dataType.length + 6),
  );
  const w = Math.min(MAX_W, Math.max(MIN_W, longest * CHAR_W + 24));
  return { w, h: HEADER_H + t.columns.length * ROW_H + PAD_BOTTOM };
}

/** Tables referenced (inside the schema), by table. */
function references(tables: DiagramTable[]): Map<string, string[]> {
  const names = new Set(tables.map((t) => t.name));
  return new Map(
    tables.map((t) => [
      t.name,
      t.foreignKeys.map((f) => f.refTable).filter((r) => names.has(r) && r !== t.name),
    ]),
  );
}

/** Layer of each table: 0 for tables that reference nothing, else one more than the
 * deepest table they reference. Cycles are cut where they close. */
export function layers(tables: DiagramTable[]): Map<string, number> {
  const refs = references(tables);
  const level = new Map<string, number>();
  const visiting = new Set<string>();
  const visit = (name: string): number => {
    const known = level.get(name);
    if (known !== undefined) return known;
    if (visiting.has(name)) return -1;
    visiting.add(name);
    const deps = (refs.get(name) ?? []).map(visit);
    visiting.delete(name);
    const l = deps.length ? Math.max(...deps) + 1 : 0;
    level.set(name, l);
    return l;
  };
  [...tables].sort((a, b) => a.name.localeCompare(b.name)).forEach((t) => visit(t.name));
  return level;
}

function gridLayout(tables: DiagramTable[]): DiagramLayout {
  const cols = Math.max(1, Math.ceil(Math.sqrt(tables.length)));
  const sizes = tables.map(tableSize);
  const colW = Math.max(...sizes.map((s) => s.w)) + GAP_X / 2;
  const out: DiagramLayout = {};
  const rowH: number[] = [];
  tables.forEach((_, i) => {
    const r = Math.floor(i / cols);
    rowH[r] = Math.max(rowH[r] ?? 0, sizes[i]?.h ?? 0);
  });
  tables.forEach((t, i) => {
    const r = Math.floor(i / cols);
    const y = rowH.slice(0, r).reduce((a, h) => a + h + GAP_Y, 0);
    out[t.name] = { x: (i % cols) * colW, y };
  });
  return out;
}

/** Positions for every table: referenced tables on the left, referencing ones to the
 * right; a grid when no table references another. */
export function autoLayout(tables: DiagramTable[]): DiagramLayout {
  const level = layers(tables);
  if (tables.length === 0) return {};
  if (![...level.values()].some((l) => l > 0)) return gridLayout(tables);
  const columns: DiagramTable[][] = [];
  const maxLevel = Math.max(...level.values());
  for (let l = 0; l <= maxLevel; l++) {
    const inLayer = tables
      .filter((t) => level.get(t.name) === l)
      .sort((a, b) => a.name.localeCompare(b.name));
    for (let i = 0; i < inLayer.length; i += MAX_PER_COLUMN) {
      columns.push(inLayer.slice(i, i + MAX_PER_COLUMN));
    }
  }
  const out: DiagramLayout = {};
  let x = 0;
  for (const col of columns) {
    let y = 0;
    for (const t of col) {
      out[t.name] = { x, y };
      y += tableSize(t).h + GAP_Y;
    }
    x += Math.max(...col.map((t) => tableSize(t).w)) + GAP_X;
  }
  return out;
}

/** Saved positions where there are some, automatic ones for the rest (new tables go
 * below everything saved). */
export function mergeLayout(tables: DiagramTable[], saved: DiagramLayout): DiagramLayout {
  const auto = autoLayout(tables);
  const known = tables.filter((t) => saved[t.name]);
  if (known.length === 0) return auto;
  const bottom = Math.max(...known.map((t) => (saved[t.name]?.y ?? 0) + tableSize(t).h));
  const out: DiagramLayout = {};
  for (const t of tables) {
    const a = auto[t.name] ?? { x: 0, y: 0 };
    out[t.name] = saved[t.name] ?? { x: a.x, y: a.y + bottom + GAP_Y * 2 };
  }
  return out;
}

export function rectOf(t: DiagramTable, layout: DiagramLayout): Rect {
  return { ...(layout[t.name] ?? { x: 0, y: 0 }), ...tableSize(t) };
}

export function bounds(rects: Rect[]): Rect {
  if (rects.length === 0) return { x: 0, y: 0, w: 0, h: 0 };
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  const w = Math.max(...rects.map((r) => r.x + r.w)) - x;
  const h = Math.max(...rects.map((r) => r.y + r.h)) - y;
  return { x, y, w, h };
}

/** Middle of a column's row on the left or right edge of its table. */
function anchor(r: Rect, row: number, right: boolean): Point {
  const y = row < 0 ? r.y + HEADER_H / 2 : r.y + HEADER_H + row * ROW_H + ROW_H / 2;
  return { x: right ? r.x + r.w : r.x, y };
}

/** A curved connector from a column of `from` to a column of `to` (row -1 = header),
 * leaving and entering on the sides that face each other. */
export function edgePath(from: Rect, fromRow: number, to: Rect, toRow: number): string {
  if (from === to || (from.x === to.x && from.y === to.y)) {
    const a = anchor(from, fromRow, true);
    const b = anchor(to, toRow, true);
    return `M ${a.x} ${a.y} C ${a.x + 40} ${a.y}, ${b.x + 40} ${b.y}, ${b.x} ${b.y}`;
  }
  const rightward = from.x + from.w / 2 <= to.x + to.w / 2;
  const a = anchor(from, fromRow, rightward);
  const b = anchor(to, toRow, !rightward);
  const bend = Math.max(30, Math.abs(b.x - a.x) / 2);
  const dir = rightward ? 1 : -1;
  return `M ${a.x} ${a.y} C ${a.x + dir * bend} ${a.y}, ${b.x - dir * bend} ${b.y}, ${b.x} ${b.y}`;
}

/** The view that shows `content` whole inside a `viewport`-sized area. */
export function fitView(content: Rect, viewport: Size, margin = 32): View {
  if (content.w <= 0 || content.h <= 0) return { x: margin, y: margin, scale: 1 };
  const scale = Math.min(
    1,
    Math.max(
      MIN_ZOOM,
      Math.min((viewport.w - margin * 2) / content.w, (viewport.h - margin * 2) / content.h),
    ),
  );
  const x = (viewport.w - content.w * scale) / 2 - content.x * scale;
  const y = (viewport.h - content.h * scale) / 2 - content.y * scale;
  return { x, y, scale };
}

/** Zooms by `factor` keeping the point `at` (viewport coordinates) still. */
export function zoomAt(v: View, factor: number, at: Point): View {
  const scale = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, v.scale * factor));
  const k = scale / v.scale;
  return { scale, x: at.x - (at.x - v.x) * k, y: at.y - (at.y - v.y) * k };
}

/** Tables linked to `name` by a foreign key, either way (and itself). */
export function related(tables: DiagramTable[], name: string): Set<string> {
  const out = new Set([name]);
  for (const t of tables) {
    for (const f of t.foreignKeys) {
      if (t.name === name) out.add(f.refTable);
      if (f.refTable === name) out.add(t.name);
    }
  }
  return out;
}
