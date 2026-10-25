// Bar and line charts in plain SVG: one y axis with round ticks, labels on x, a legend
// when there are several series, and a native tooltip on every mark.

import { type ValueFormat } from "@/types";
import { band, type ChartData, extent, formatValue, linear, niceTicks } from "../chartScale";
import { useSize } from "../useSize";
import { Legend } from "./Legend";

const M = { top: 10, right: 12, bottom: 24, left: 46 };
/** Series beyond the palette fold away rather than reuse a color. */
export const MAX_SERIES = 8;

interface Plot {
  w: number;
  h: number;
  x0: number;
  y: (v: number) => number;
}

function YAxis({ p, ticks, format }: { p: Plot; ticks: number[]; format: ValueFormat }) {
  return (
    <g className="chart-axis">
      {ticks.map((t) => (
        <g key={t} transform={`translate(0 ${p.y(t)})`}>
          <line className={t === 0 ? "chart-zero" : "chart-grid"} x1={M.left} x2={p.w - M.right} />
          <text x={M.left - 6} y={3.5} textAnchor="end">
            {formatValue(t, format, true)}
          </text>
        </g>
      ))}
    </g>
  );
}

/** Every n-th label so they don't collide. */
function XLabels({
  labels,
  x,
  width,
}: {
  labels: string[];
  x: (i: number) => number;
  width: number;
}) {
  const every = Math.max(1, Math.ceil((labels.length * 64) / Math.max(1, width)));
  return (
    <g className="chart-axis">
      {labels.map((l, i) =>
        i % every === 0 ? (
          <text key={i} x={x(i)} y={M.bottom - 8} textAnchor="middle">
            {l.length > 12 ? `${l.slice(0, 11)}…` : l}
          </text>
        ) : null,
      )}
    </g>
  );
}

function Bars({ data, p, format }: { data: ChartData; p: Plot; format: ValueFormat }) {
  const n = data.series.length;
  const b = band(data.labels.length, p.w - M.left - M.right, 0.25);
  const inner = Math.max(1, (b.width - (n - 1) * 2) / n);
  return (
    <g>
      {data.series.map((s, si) =>
        s.values.map((v, i) => {
          if (v === null) return null;
          const x = M.left + b.x(i) + si * (inner + 2);
          const [y0, y1] = [p.y(Math.max(0, v)), p.y(Math.min(0, v))];
          return (
            <rect
              key={`${si}-${i}`}
              className={`chart-bar chart-s${si + 1}`}
              x={x}
              y={y0}
              width={inner}
              height={Math.max(1, y1 - y0)}
              rx={Math.min(4, inner / 2)}
            >
              <title>{`${data.labels[i]} · ${s.name}: ${formatValue(v, format)}`}</title>
            </rect>
          );
        }),
      )}
    </g>
  );
}

function Lines({ data, p, format }: { data: ChartData; p: Plot; format: ValueFormat }) {
  const span = p.w - M.left - M.right;
  const n = data.labels.length;
  const x = (i: number) => M.left + (n <= 1 ? span / 2 : (i / (n - 1)) * span);
  return (
    <g>
      {data.series.map((s, si) => {
        const pts = s.values.flatMap((v, i) => (v === null ? [] : [[x(i), p.y(v), v, i] as const]));
        const d = pts.map(([px, py], k) => `${k ? "L" : "M"} ${px} ${py}`).join(" ");
        return (
          <g key={si} className={`chart-s${si + 1}`}>
            <path className="chart-line" d={d} />
            {pts.map(([px, py, v, i]) => (
              <circle key={i} className="chart-dot" cx={px} cy={py} r={n > 40 ? 2 : 4}>
                <title>{`${data.labels[i]} · ${s.name}: ${formatValue(v, format)}`}</title>
              </circle>
            ))}
          </g>
        );
      })}
    </g>
  );
}

export function XYChart(props: { data: ChartData; mode: "bar" | "line"; format: ValueFormat }) {
  const size = useSize<HTMLDivElement>();
  const data = { ...props.data, series: props.data.series.slice(0, MAX_SERIES) };
  const [lo, hi] = extent(data.series);
  const t = niceTicks(lo, hi, Math.max(2, Math.floor(size.h / 40)));
  const w = Math.max(120, size.w);
  const h = Math.max(80, size.h);
  const p: Plot = { w, h, x0: M.left, y: linear(t.min, t.max, h - M.bottom, M.top) };
  const span = w - M.left - M.right;
  const b = band(data.labels.length, span, 0.25);
  const n = data.labels.length;
  const labelX =
    props.mode === "bar"
      ? (i: number) => M.left + b.x(i) + b.width / 2
      : (i: number) => M.left + (n <= 1 ? span / 2 : (i / (n - 1)) * span);
  return (
    <div className="chart">
      <div className="chart-plot" ref={size.ref}>
        <svg width={w} height={h} role="img" aria-label={`${props.mode} chart`}>
          <YAxis p={p} ticks={t.ticks} format={props.format} />
          {props.mode === "bar" ? (
            <Bars data={data} p={p} format={props.format} />
          ) : (
            <Lines data={data} p={p} format={props.format} />
          )}
          <g transform={`translate(0 ${h - M.bottom + 8})`}>
            <XLabels labels={data.labels} x={labelX} width={span} />
          </g>
        </svg>
      </div>
      {data.series.length > 1 && <Legend names={data.series.map((s) => s.name)} />}
    </div>
  );
}
