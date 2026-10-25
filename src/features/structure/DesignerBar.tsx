// The bar under the designer: what is pending, and Discard / Review & apply.

import { Save, Undo2 } from "lucide-react";
import { BusyIcon } from "@/components/Spinner";
import { withShortcut } from "@/lib/actions";
import { pluralWord } from "@/lib/format";
import { type TableDesignerState } from "./useTableDesigner";

function Status({ d }: { d: TableDesignerState }) {
  if (d.problems.length) return <span className="warn-text">{d.problems[0]}</span>;
  if (d.creating) return <span className="muted">Review the SQL before the table is created.</span>;
  return (
    <span className="muted">
      {d.pending} {pluralWord(d.pending, "change")} not applied yet
    </span>
  );
}

export function DesignerBar({ d }: { d: TableDesignerState }) {
  if (!d.creating && d.pending === 0) return null;
  return (
    <div className="designer-bar">
      <Status d={d} />
      <span className="spacer" />
      <button className="btn sm" onClick={d.discard} disabled={d.busy}>
        <Undo2 size={13} /> {d.creating ? "Reset" : "Discard"}
      </button>
      <button
        className="btn sm primary"
        onClick={() => void d.apply()}
        disabled={d.busy || d.problems.length > 0 || !d.change}
        title={withShortcut("Review SQL", "save")}
      >
        <BusyIcon busy={d.busy} icon={<Save size={13} />} />{" "}
        {d.creating ? "Create table…" : "Review & apply…"}
      </button>
    </div>
  );
}
