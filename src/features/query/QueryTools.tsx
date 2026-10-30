// Toolbar buttons for Explain (with "Explain analyze" in its menu) and Beautify (with the
// editor's text tools in its menu: trim, indentation, transforms).

import { ChevronDown, ListTree, WandSparkles } from "lucide-react";
import { SEPARATOR, useContextMenu } from "@/components/ContextMenu";
import { withShortcut } from "@/lib/actions";
import { TRANSFORMS, type TextTransform } from "./editor/editing";

/** Text tools of the editor, from the Format menu. */
export type EditorTool =
  | { kind: "trim" }
  | { kind: "indent"; to: "spaces" | "tabs" }
  | { kind: "transform"; transform: TextTransform };

export interface QueryToolsProps {
  running: boolean;
  /** The driver can explain statements (EXPLAIN). */
  canExplain: boolean;
  /** The editor holds SQL the formatter knows. */
  canFormat: boolean;
  canAnalyze: boolean;
  onExplain: (analyze: boolean) => void;
  onFormat: () => void;
  onTool: (t: EditorTool) => void;
}

export function QueryTools(p: QueryToolsProps) {
  const menu = useContextMenu();
  const items = [
    { label: "Explain", onClick: () => p.onExplain(false) },
    { label: "Explain analyze…", onClick: () => p.onExplain(true) },
  ];
  const textItems = [
    ...(p.canFormat ? [{ label: "Beautify SQL", onClick: p.onFormat }, SEPARATOR] : []),
    { label: "Trim trailing whitespace", onClick: () => p.onTool({ kind: "trim" }) },
    {
      label: "Convert indentation",
      submenu: [
        { label: "To spaces", onClick: () => p.onTool({ kind: "indent", to: "spaces" }) },
        { label: "To tabs", onClick: () => p.onTool({ kind: "indent", to: "tabs" }) },
      ],
    },
    {
      label: "Transform selection",
      submenu: TRANSFORMS.map(([transform, label]) => ({
        label,
        onClick: () => p.onTool({ kind: "transform", transform }),
      })),
    },
  ];
  return (
    <div className="tool-group">
      {p.canExplain && (
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
      )}
      <div className="split-btn">
        {p.canFormat && (
          <button
            className="btn sm"
            onClick={p.onFormat}
            title={withShortcut("Beautify the selection, or all", "format-sql")}
          >
            <WandSparkles size={13} /> Format
          </button>
        )}
        <button
          className="btn sm icon"
          aria-label="Text tools"
          title="Trim whitespace, convert indentation, transform text"
          onClick={(e) => menu.openAt(e.currentTarget, textItems)}
        >
          <ChevronDown size={12} />
        </button>
      </div>
      {menu.node}
    </div>
  );
}
