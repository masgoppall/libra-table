import { type ReactNode } from "react";

/** The disclosure chevron of the sidebar tree (8 × 4.5 pt open); it turns as it opens. */
export function Disclosure({ open }: { open: boolean }) {
  return (
    <svg
      className={`side-disclosure${open ? " open" : ""}`}
      width="10"
      height="10"
      viewBox="0 0 10 10"
      aria-hidden
    >
      <path
        d="M3.2 1 7.2 5 3.2 9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** A collapsible group of the sidebar tree ("Functions", "Tables"…) with an optional "+". */
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
      <div
        className="side-section"
        role="treeitem"
        aria-expanded={!props.collapsed}
        title={`${props.count} ${props.label.toLowerCase()}`}
        onClick={props.onToggle}
        onContextMenu={props.onMenu}
      >
        <Disclosure open={!props.collapsed} />
        <span className="side-section-label">{props.label}</span>
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

/** A message in the middle of the sidebar ("No database selected…"). */
export function SideEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="side-empty">
      <p>{children}</p>
    </div>
  );
}
