// Toolbar buttons for Explain (with "Explain analyze" in its menu) and Beautify.

import { ChevronDown, ListTree, WandSparkles } from "lucide-react";
import { useContextMenu } from "@/components/ContextMenu";
import { withShortcut } from "@/lib/actions";

export interface QueryToolsProps {
  running: boolean;
  canAnalyze: boolean;
  onExplain: (analyze: boolean) => void;
  onFormat: () => void;
}

export function QueryTools(p: QueryToolsProps) {
  const menu = useContextMenu();
  const items = [
    { label: "Explain", onClick: () => p.onExplain(false) },
    { label: "Explain analyze…", onClick: () => p.onExplain(true) },
  ];
  return (
    <div className="tool-group">
      <div className="split-btn">
        <button
          className="btn sm"
          onClick={() => p.onExplain(false)}
          disabled={p.running}
          title={withShortcut("Explain the statement at the cursor", "explain")}
        >
          <ListTree size={13} /> Explain
        </button>
        {p.canAnalyze && (
          <button
            className="btn sm icon"
            disabled={p.running}
            aria-label="More explain options"
            onClick={(e) => menu.openAt(e.currentTarget, items)}
          >
            <ChevronDown size={12} />
          </button>
        )}
      </div>
      <button
        className="btn sm"
        onClick={p.onFormat}
        title={withShortcut("Beautify the selection, or all", "format-sql")}
      >
        <WandSparkles size={13} /> Format
      </button>
      {menu.node}
    </div>
  );
}
