// Metrics boards: dashboards of widgets that each run a read-only query.

export type WidgetKind = "scoreboard" | "table" | "bar" | "line" | "pie" | "input";

export type ValueFormat = "auto" | "number" | "percent" | "currency" | "bytes";

export interface WidgetOptions {
  /** Charts: x axis / slice labels ("" = first column). */
  labelColumn: string;
  /** Charts: value columns (empty = every numeric column but the label). */
  valueColumns: string[];
  /** Scoreboard: text under the value. */
  label: string;
  format: ValueFormat;
  /** Input: the `{{name}}` other widgets use. */
  variable: string;
  defaultValue: string;
  inputType: "text" | "number";
}

export interface MetricWidget {
  id: string;
  title: string;
  kind: WidgetKind;
  sql: string;
  /** Grid columns (1–4) and rows (1–3). */
  width: number;
  height: number;
  options: WidgetOptions;
}

export interface MetricBoard {
  id: string;
  name: string;
  /** 0 = no automatic refresh. */
  refreshSeconds: number;
  widgets: MetricWidget[];
}

export interface MetricVariable {
  name: string;
  value: string;
  kind: "text" | "number";
}
