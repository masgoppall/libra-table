import { ChevronDown, ChevronRight, Eye, Key, Pin, Table2 } from "lucide-react";
import { Highlight } from "@/components/Highlight";
import { Spinner } from "@/components/Spinner";
import { type ObjectInfo } from "@/types";
import { type ColumnsState } from "./useTableColumns";

export interface ObjectItemProps {
  object: ObjectInfo;
  indexes: number[];
  active: boolean;
  expanded: boolean;
  columns: ColumnsState | undefined;
  onOpen: () => void;
  onToggle: () => void;
  onMenu: (e: React.MouseEvent) => void;
  pinned?: boolean;
}

function Columns({ columns }: { columns: ColumnsState | undefined }) {
  if (columns === "loading") {
    return (
      <div className="side-col">
        <Spinner size={11} /> loading
      </div>
    );
  }
  return (
    <>
      {columns?.map((c) => (
        <div
          key={c.name}
          className="side-col"
          title={`${c.name} ${c.dataType}${c.nullable ? "" : " NOT NULL"}`}
        >
          {c.pkPosition ? <Key size={10} color="var(--warn)" /> : <span className="icon-gap" />}
          <span className="side-col-name">{c.name}</span>
          <span className="type">{c.dataType}</span>
        </div>
      ))}
    </>
  );
}

/** A table or view; the caret shows its columns. */
export function ObjectItem(p: ObjectItemProps) {
  const o = p.object;
  return (
    <div>
      <div
        className={`side-item${p.active ? " active" : ""}`}
        onClick={p.onOpen}
        onContextMenu={p.onMenu}
        title={o.name}
      >
        <span
          className="side-caret"
          onClick={(e) => {
            e.stopPropagation();
            p.onToggle();
          }}
        >
          {p.expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
        {o.kind === "view" ? (
          <Eye size={13} className="side-icon view" />
        ) : (
          <Table2 size={13} className="side-icon table" />
        )}
        <span className="label">
          <Highlight text={o.name} indexes={p.indexes} />
        </span>
        {p.pinned && <Pin size={11} className="side-pin" />}
      </div>
      {p.expanded && <Columns columns={p.columns} />}
    </div>
  );
}
