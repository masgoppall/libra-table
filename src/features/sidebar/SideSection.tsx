import { ChevronDown, ChevronRight } from "lucide-react";
import { type ReactNode } from "react";

/** A collapsible sidebar section with a count and an optional "+" action. */
export function SideSection(props: {
  label: string;
  count: number;
  collapsed: boolean;
  onToggle: () => void;
  action?: ReactNode;
  /** Context menu of the section header. */
  onMenu?: (e: React.MouseEvent) => void;
  children: ReactNode;
}) {
  return (
    <div className="side-group">
      <div className="side-section" onClick={props.onToggle} onContextMenu={props.onMenu}>
        {props.collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>{props.label}</span>
        <span className="side-count">{props.count}</span>
        <span className="spacer" />
        {props.action && (
          <span className="side-section-action" onClick={(e) => e.stopPropagation()}>
            {props.action}
          </span>
        )}
      </div>
      {!props.collapsed && props.children}
    </div>
  );
}

export function SideHint({ children }: { children: ReactNode }) {
  return <div className="side-hint">{children}</div>;
}
