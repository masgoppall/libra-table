import { RotateCcw, Trash2 } from "lucide-react";

/** Drop (or keep again) a row of the designer. */
export function DropButton(props: { dropped: boolean; what: string; onClick: () => void }) {
  return (
    <button
      className="btn ghost icon xs"
      title={props.dropped ? "Keep" : `Drop this ${props.what}`}
      onClick={props.onClick}
    >
      {props.dropped ? <RotateCcw size={12} /> : <Trash2 size={12} />}
    </button>
  );
}
