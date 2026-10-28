// The table's CREATE statement, from the bottom bar's DDL button.

import { Sheet } from "@/components/Sheet";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";

export function DdlSheet(p: { table: string; ddl: string; onClose: () => void }) {
  const copy = async () => {
    await copyText(p.ddl);
    toastSuccess("DDL copied.");
  };
  return (
    <Sheet
      label={`DDL of ${p.table}`}
      className="ddl-sheet"
      onClose={p.onClose}
      footer={
        <>
          <button className="sheet-btn" onClick={() => void copy()}>
            Copy
          </button>
          <span className="spacer" />
          <button className="sheet-btn primary" onClick={p.onClose}>
            Done
          </button>
        </>
      }
    >
      <div className="ddl-sheet-title">{p.table}</div>
      <pre className="ddl-sheet-code mono selectable">{p.ddl}</pre>
    </Sheet>
  );
}
