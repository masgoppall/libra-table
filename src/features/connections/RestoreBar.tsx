import { RotateCcw } from "lucide-react";
import { Spinner } from "@/components/Spinner";
import { connectionColor } from "@/domain/connection";
import { type ConnectionView } from "@/types";

/** STO-5: connections open last time, one click away (nothing reconnects by itself). */
export function RestoreBar(props: {
  connections: ConnectionView[];
  connecting: string | null;
  onConnect: (c: ConnectionView) => void;
}) {
  if (!props.connections.length) return null;
  return (
    <div className="restore">
      <RotateCcw size={14} className="muted" />
      <span className="muted">Last session had open:</span>
      {props.connections.map((c) => (
        <button
          key={c.id}
          className="btn sm"
          onClick={() => props.onConnect(c)}
          disabled={props.connecting === c.id}
        >
          {props.connecting === c.id ? (
            <Spinner size={12} />
          ) : (
            <span className="dot" style={{ background: connectionColor(c) }} />
          )}{" "}
          {c.name}
        </button>
      ))}
      <span className="faint small">Nothing reconnects until you click.</span>
    </div>
  );
}
