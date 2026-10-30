// The "Jump to column or row" sheet over a grid: type a column name, a row number or a
// value; Enter (or a double-click) moves the grid's focus there.

import { Columns3, Hash, TextSearch } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Highlight } from "@/components/Highlight";
import { Sheet, SheetSearch } from "@/components/Sheet";
import { type CellPos } from "@/components/grid/types";
import { jumpTargets, type JumpSource, type JumpTarget } from "./jump";

function targetKey(t: JumpTarget): string {
  if (t.kind === "column") return `c${t.col}`;
  if (t.kind === "row") return `r${t.row}`;
  return `v${t.row}:${t.col}`;
}

function TargetRow({ t }: { t: JumpTarget }) {
  if (t.kind === "column") {
    return (
      <>
        <Columns3 size={14} className="dbs-icon" />
        <span className="dbs-name">
          <Highlight text={t.name} indexes={t.match.indexes} />
        </span>
        <span className="dbs-current">column</span>
      </>
    );
  }
  if (t.kind === "row") {
    return (
      <>
        <Hash size={14} className="dbs-icon" />
        <span className="dbs-name">Go to row {t.label} on this page</span>
      </>
    );
  }
  return (
    <>
      <TextSearch size={14} className="dbs-icon" />
      <span className="dbs-name">{t.text.length > 80 ? `${t.text.slice(0, 80)}…` : t.text}</span>
      <span className="dbs-current">{t.column}</span>
    </>
  );
}

export function JumpSheet(props: {
  source: JumpSource;
  /** The focused cell's row, kept when jumping to a column. */
  focusRow: number;
  onJump: (to: CellPos) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const targets = useMemo(() => jumpTargets(props.source, query), [props.source, query]);
  const current = Math.min(selected, targets.length - 1);
  useEffect(() => setSelected(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector(".dbs-row.selected")?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  const jump = (t: JumpTarget | undefined) => {
    if (!t) return;
    const first = props.source.visible[0] ?? 0;
    props.onClose();
    if (t.kind === "column") props.onJump({ row: props.focusRow, col: t.col });
    else if (t.kind === "row") props.onJump({ row: t.row, col: first });
    else props.onJump({ row: t.row, col: t.col });
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (step) {
      e.preventDefault();
      setSelected(Math.max(0, Math.min(targets.length - 1, current + step)));
    } else if (e.key === "Enter") {
      e.preventDefault();
      jump(targets[current]);
    }
  };

  return (
    <Sheet
      label="Jump to column or row"
      className="dbs-sheet"
      onClose={props.onClose}
      footer={
        <>
          <span className="faint jump-hint">Start with &gt; to search the values only</span>
          <span className="spacer" />
          <button className="sheet-btn" onClick={props.onClose}>
            Cancel
          </button>
          <button
            className="sheet-btn primary"
            onClick={() => jump(targets[current])}
            disabled={!targets.length}
          >
            Go
          </button>
        </>
      }
    >
      <div className="dbs-title">Jump to Column or Row</div>
      <div className="dbs-body" onKeyDown={onKeyDown}>
        <SheetSearch value={query} onChange={setQuery} placeholder="Column, row number, or value" />
        <div className="dbs-list" ref={listRef} role="listbox" aria-label="Jump targets">
          {targets.length === 0 && <div className="dbs-state">No matching columns or rows</div>}
          {targets.map((t, i) => (
            <div
              key={targetKey(t)}
              role="option"
              aria-selected={i === current}
              className={`dbs-row${i === current ? " selected" : ""}`}
              onClick={() => setSelected(i)}
              onDoubleClick={() => jump(t)}
            >
              <TargetRow t={t} />
            </div>
          ))}
        </div>
      </div>
    </Sheet>
  );
}

/** Where the grid should move its focus to: `go` jumps (again) to a cell. */
export function useReveal() {
  const [reveal, setReveal] = useState<{ at: CellPos; seq: number }>();
  const go = (at: CellPos) => setReveal((r) => ({ at, seq: (r?.seq ?? 0) + 1 }));
  return { reveal, go };
}

/** The sheet, opened with a snapshot of the grid. */
export function useJumpSheet(go: (at: CellPos) => void) {
  const [open, setOpen] = useState<{ source: JumpSource; focusRow: number } | null>(null);
  const node = open && (
    <JumpSheet
      source={open.source}
      focusRow={open.focusRow}
      onJump={go}
      onClose={() => setOpen(null)}
    />
  );
  const show = (source: JumpSource, focusRow: number | null) =>
    setOpen({ source, focusRow: focusRow ?? 0 });
  return { show, node };
}
