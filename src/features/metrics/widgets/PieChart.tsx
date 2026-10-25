// A donut of the first value column: up to seven slices, the rest folded into "Other".

import { type ValueFormat } from "@/types";
import { arcPath, type ChartData, formatValue, pieSlices } from "../chartScale";
import { useSize } from "../useSize";
import { Legend } from "./Legend";

const MAX_SLICES = 7;

function fold(labels: string[], values: (number | null)[]) {
  const items = labels
    .map((l, i) => ({ l, v: values[i] ?? 0 }))
    .filter((x) => x.v > 0)
    .sort((a, b) => b.v - a.v);
  if (items.length <= MAX_SLICES + 1) return items;
  const rest = items.slice(MAX_SLICES).reduce((a, x) => a + x.v, 0);
  return [...items.slice(0, MAX_SLICES), { l: "Other", v: rest }];
}

export function PieChart({ data, format }: { data: ChartData; format: ValueFormat }) {
  const size = useSize<HTMLDivElement>();
  const items = fold(data.labels, data.series[0]?.values ?? []);
  const slices = pieSlices(
    items.map((x) => x.l),
    items.map((x) => x.v),
  );
  const d = Math.max(60, Math.min(size.w, size.h));
  const r = d / 2 - 4;
  const total = items.reduce((a, x) => a + x.v, 0);
  return (
    <div className="chart">
      <div className="chart-plot center" ref={size.ref}>
        <svg width={d} height={d} role="img" aria-label="pie chart">
          {slices.map((s, i) => (
            <path
              key={s.label}
              className={`chart-slice chart-s${i + 1}`}
              d={arcPath(d / 2, d / 2, r, r * 0.58, s.start, s.end)}
            >
              <title>{`${s.label}: ${formatValue(s.value, format)}`}</title>
            </path>
          ))}
          <text className="chart-total" x={d / 2} y={d / 2 + 5} textAnchor="middle">
            {formatValue(total, format, true)}
          </text>
        </svg>
      </div>
      <Legend names={slices.map((s) => s.label)} />
    </div>
  );
}
