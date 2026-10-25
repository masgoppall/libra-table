// Editing boards without UI: new boards and widgets, reorder, resize, and the
// `{{variables}}` input widgets provide.

import { uid } from "@/lib/uid";
import {
  type MetricBoard,
  type MetricVariable,
  type MetricWidget,
  type WidgetKind,
  type WidgetOptions,
} from "@/types";

export const WIDGET_KINDS: [WidgetKind, string][] = [
  ["scoreboard", "Scoreboard"],
  ["table", "Table"],
  ["bar", "Bar chart"],
  ["line", "Line chart"],
  ["pie", "Pie chart"],
  ["input", "Input"],
];

export const REFRESH_CHOICES: [number, string][] = [
  [0, "Off"],
  [10, "10 s"],
  [30, "30 s"],
  [60, "1 min"],
  [300, "5 min"],
  [900, "15 min"],
];

export const DEFAULT_OPTIONS: WidgetOptions = {
  labelColumn: "",
  valueColumns: [],
  label: "",
  format: "auto",
  variable: "",
  defaultValue: "",
  inputType: "text",
};

export function newBoard(name: string): MetricBoard {
  return { id: uid(), name, refreshSeconds: 0, widgets: [] };
}

const SIZES: Record<WidgetKind, [number, number]> = {
  scoreboard: [1, 1],
  input: [1, 1],
  table: [2, 2],
  bar: [2, 2],
  line: [2, 2],
  pie: [1, 2],
};

export function newWidget(kind: WidgetKind, taken: string[] = []): MetricWidget {
  const [width, height] = SIZES[kind];
  const label = WIDGET_KINDS.find(([k]) => k === kind)?.[1] ?? kind;
  let variable = "";
  if (kind === "input") {
    let n = 1;
    while (taken.includes(`value${n}`)) n++;
    variable = `value${n}`;
  }
  return {
    id: uid(),
    title: kind === "input" ? variable : label,
    kind,
    sql: kind === "input" ? "" : "SELECT 1 AS value",
    width,
    height,
    options: { ...DEFAULT_OPTIONS, variable },
  };
}

export function updateWidget(b: MetricBoard, w: MetricWidget): MetricBoard {
  return { ...b, widgets: b.widgets.map((x) => (x.id === w.id ? w : x)) };
}

export function removeWidget(b: MetricBoard, id: string): MetricBoard {
  return { ...b, widgets: b.widgets.filter((x) => x.id !== id) };
}

/** Moves a widget one place earlier (-1) or later (1) in the grid. */
export function moveWidget(b: MetricBoard, id: string, dir: -1 | 1): MetricBoard {
  const i = b.widgets.findIndex((w) => w.id === id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= b.widgets.length) return b;
  const widgets = [...b.widgets];
  [widgets[i], widgets[j]] = [widgets[j] as MetricWidget, widgets[i] as MetricWidget];
  return { ...b, widgets };
}

export function resizeWidget(b: MetricBoard, id: string, dw: number, dh: number): MetricBoard {
  const clamp = (v: number, max: number) => Math.min(max, Math.max(1, v));
  return {
    ...b,
    widgets: b.widgets.map((w) =>
      w.id === id ? { ...w, width: clamp(w.width + dw, 4), height: clamp(w.height + dh, 3) } : w,
    ),
  };
}

/** `{{name}}` placeholders a query uses (Rust skips those in strings and comments). */
export function variableNames(sql: string): string[] {
  const names = [...sql.matchAll(/\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g)].map((m) => m[1]);
  return [...new Set(names.filter((n): n is string => !!n))];
}

/** The board's input values as variables: the typed value, else the default. */
export function boardVariables(b: MetricBoard, values: Record<string, string>): MetricVariable[] {
  return b.widgets
    .filter((w) => w.kind === "input" && w.options.variable)
    .map((w) => ({
      name: w.options.variable,
      value: values[w.options.variable] ?? w.options.defaultValue,
      kind: w.options.inputType,
    }));
}
