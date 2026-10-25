// Chart math without UI: nice axis ticks, linear and band scales, query results as
// series, pie arcs and value formats.

import { type Cell, type StatementResult, type ValueFormat, type WidgetOptions } from "@/types";

export interface Ticks {
  min: number;
  max: number;
  step: number;
  ticks: number[];
}

/** 1, 2, 2.5 or 5 times a power of ten, at least `raw`. */
function niceStep(raw: number): number {
  const pow = 10 ** Math.floor(Math.log10(raw));
  const f = raw / pow;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
  return nice * pow;
}

/** About `count` round ticks covering [min, max]; zero is always included. */
export function niceTicks(min: number, max: number, count = 5): Ticks {
  let lo = Math.min(0, min);
  let hi = Math.max(0, max);
  if (lo === hi) hi = lo + 1;
  const step = niceStep((hi - lo) / Math.max(1, count));
  lo = Math.floor(lo / step) * step;
  hi = Math.ceil(hi / step) * step;
  const ticks: number[] = [];
  for (let v = lo; v <= hi + step / 1e6; v += step) ticks.push(Number(v.toPrecision(12)));
  return { min: lo, max: hi, step, ticks };
}

/** Maps [d0, d1] onto [r0, r1]. */
export function linear(d0: number, d1: number, r0: number, r1: number): (v: number) => number {
  const span = d1 - d0 || 1;
  return (v) => r0 + ((v - d0) / span) * (r1 - r0);
}

/** `n` bands across `width`: the band width and each band's left edge. */
export function band(n: number, width: number, padding = 0.2) {
  const step = width / Math.max(1, n);
  const inner = step * (1 - padding);
  return { step, width: inner, x: (i: number) => i * step + (step - inner) / 2 };
}

export function toNumber(c: Cell | undefined): number | null {
  if (typeof c === "number") return Number.isFinite(c) ? c : null;
  if (typeof c === "boolean") return c ? 1 : 0;
  if (typeof c === "string" && c.trim() !== "" && Number.isFinite(Number(c))) return Number(c);
  return null;
}

export function cellText(c: Cell | undefined): string {
  if (c === null || c === undefined) return "NULL";
  if (typeof c === "object") return `<binary ${c.size} bytes>`;
  return String(c);
}

export interface Series {
  name: string;
  values: (number | null)[];
}

export interface ChartData {
  labels: string[];
  series: Series[];
}

/** The label column and value columns of a result, as chart series. Without explicit
 * value columns, every column whose values are numbers (except the label) is one. */
export function chartData(r: StatementResult, o: WidgetOptions): ChartData {
  const names = r.columns.map((c) => c.name);
  const labelAt = Math.max(0, o.labelColumn ? names.indexOf(o.labelColumn) : 0);
  const numeric = (i: number) => r.rows.some((row) => toNumber(row[i]) !== null);
  const valueAt = o.valueColumns.length
    ? o.valueColumns.map((v) => names.indexOf(v)).filter((i) => i >= 0)
    : names.map((_, i) => i).filter((i) => i !== labelAt && numeric(i));
  return {
    labels: r.rows.map((row) => cellText(row[labelAt])),
    series: valueAt.map((i) => ({
      name: names[i] ?? "",
      values: r.rows.map((row) => toNumber(row[i])),
    })),
  };
}

/** Lowest and highest value of all series (0, 0 when there are none). */
export function extent(series: Series[]): [number, number] {
  const all = series.flatMap((s) => s.values.filter((v): v is number => v !== null));
  return all.length ? [Math.min(...all), Math.max(...all)] : [0, 0];
}

export interface Slice {
  label: string;
  value: number;
  start: number;
  end: number;
}

/** Pie slices (angles in radians from 12 o'clock); negative and empty values drop out. */
export function pieSlices(labels: string[], values: (number | null)[]): Slice[] {
  const items = labels
    .map((label, i) => ({ label, value: values[i] ?? 0 }))
    .filter((s) => s.value > 0);
  const total = items.reduce((a, s) => a + s.value, 0);
  let at = 0;
  return items.map((s) => {
    const start = at;
    at += (s.value / total) * Math.PI * 2;
    return { ...s, start, end: at };
  });
}

function point(cx: number, cy: number, r: number, a: number): string {
  return `${(cx + r * Math.sin(a)).toFixed(2)} ${(cy - r * Math.cos(a)).toFixed(2)}`;
}

/** A donut segment from angle `a0` to `a1`. */
export function arcPath(cx: number, cy: number, r: number, inner: number, a0: number, a1: number) {
  // A full circle can't be one arc: stop just short of it.
  const end = a1 - a0 >= Math.PI * 2 ? a0 + Math.PI * 2 - 1e-4 : a1;
  const large = end - a0 > Math.PI ? 1 : 0;
  return [
    `M ${point(cx, cy, r, a0)}`,
    `A ${r} ${r} 0 ${large} 1 ${point(cx, cy, r, end)}`,
    `L ${point(cx, cy, inner, end)}`,
    `A ${inner} ${inner} 0 ${large} 0 ${point(cx, cy, inner, a0)}`,
    "Z",
  ].join(" ");
}

const compact = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 });
const plain = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });

export function formatValue(v: number | null, format: ValueFormat, short = false): string {
  if (v === null) return "–";
  switch (format) {
    case "percent":
      return `${plain.format(v * 100)}%`;
    case "currency":
      return `$${(short ? compact : plain).format(v)}`;
    case "bytes": {
      const units = ["B", "KB", "MB", "GB", "TB"];
      let n = Math.abs(v);
      let u = 0;
      while (n >= 1024 && u < units.length - 1) {
        n /= 1024;
        u++;
      }
      return `${v < 0 ? "-" : ""}${plain.format(n)} ${units[u]}`;
    }
    default:
      return (short || Math.abs(v) >= 1e6 ? compact : plain).format(v);
  }
}
