// The rows of the process list, with Cancel query / Kill per row.

import { CircleStop, Skull } from "lucide-react";
import { type ProcessAction, type ProcessInfo } from "@/types";
import { formatAge, isIdle } from "./processModel";

function ProcessRow(props: {
  p: ProcessInfo;
  onStop: (p: ProcessInfo, action: ProcessAction) => void;
}) {
  const { p } = props;
  return (
    <tr className={p.own ? "own" : undefined}>
      <td className="num mono">{p.id}</td>
      <td>{p.user}</td>
      <td>{p.database}</td>
      <td className="mono faint" title={p.application || undefined}>
        {p.client}
      </td>
      <td className={isIdle(p) ? "faint" : undefined}>
        {p.state}
        {p.own && <span className="badge xs own-badge">LibraTable</span>}
      </td>
      <td className="num">{formatAge(p.durationMs)}</td>
      <td className="process-query mono" title={p.query}>
        {p.query}
      </td>
      <td className="process-actions">
        <button
          className="btn xs"
          title="Cancel the running query"
          disabled={!p.query || isIdle(p)}
          onClick={() => props.onStop(p, "cancel")}
        >
          <CircleStop size={12} /> Cancel
        </button>
        <button
          className="btn xs danger-outline"
          title="Close the connection"
          onClick={() => props.onStop(p, "kill")}
        >
          <Skull size={12} /> Kill
        </button>
      </td>
    </tr>
  );
}

export function ProcessTable(props: {
  rows: ProcessInfo[];
  idLabel: string;
  onStop: (p: ProcessInfo, action: ProcessAction) => void;
}) {
  return (
    <table className="edit-table server-table">
      <thead>
        <tr>
          <th className="col-num">{props.idLabel}</th>
          <th>User</th>
          <th>Database</th>
          <th>Client</th>
          <th>State</th>
          <th>Time</th>
          <th>Query</th>
          <th className="col-action" />
        </tr>
      </thead>
      <tbody>
        {props.rows.map((p) => (
          <ProcessRow key={p.id} p={p} onStop={props.onStop} />
        ))}
      </tbody>
    </table>
  );
}
