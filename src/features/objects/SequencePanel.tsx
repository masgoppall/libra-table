import { type SequenceInfo } from "@/types";

/** A sequence's current value and settings. */
export function SequencePanel({ info }: { info: SequenceInfo }) {
  const rows: [string, string][] = [
    ["Last value", info.lastValue ?? "not used yet"],
    ["Increment", info.increment],
    ["Start", info.startValue],
    ["Min / max", `${info.minValue} / ${info.maxValue}`],
    ["Cache", info.cache],
    ["Cycles", info.cycle ? "yes" : "no"],
    ["Type", info.dataType],
  ];
  return (
    <table className="plain facts">
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <th>{k}</th>
            <td className="mono selectable">{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
