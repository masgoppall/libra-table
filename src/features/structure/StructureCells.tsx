// Cells of the structure grids, drawn as TablePlus draws them (monospaced values, grey
// NULL / EMPTY, ⌃⌄ on popup cells) and edited in place.

import { useState } from "react";
import { cx } from "@/lib/cx";
import { type CellText } from "./structureColumns";

/** The ⌃⌄ of a popup cell (6 × 11 pt). */
export function Chevron() {
  return (
    <svg width="7" height="11" viewBox="0 0 7 11" aria-hidden>
      <path
        d="M1 3.6 3.5 1.2 6 3.6M1 7.4l2.5 2.4L6 7.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** The → of a foreign_key cell. */
export function Arrow() {
  return (
    <svg width="9" height="8" viewBox="0 0 9 8" aria-hidden>
      <path
        d="M.8 4h6.6M4.8 1.2 7.6 4 4.8 6.8"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** The drag handle of a row: two columns of three rings. */
export function Grip() {
  return (
    <svg width="9" height="13" viewBox="0 0 9 13" aria-hidden>
      {[0, 1, 2].flatMap((row) =>
        [0, 1].map((col) => (
          <circle
            key={`${row}${col}`}
            cx={2 + col * 5}
            cy={2.2 + row * 4.3}
            r="1.35"
            fill="none"
            stroke="currentColor"
            strokeWidth="0.9"
          />
        )),
      )}
    </svg>
  );
}

const FIT_PATH = [
  "M5.4 5.9V3.7a1.6 1.6 0 0 1 1.6-1.6h2a1.6 1.6 0 0 1 1.6 1.6v2.2",
  "M5.4 10.1v2.2a1.6 1.6 0 0 0 1.6 1.6h2a1.6 1.6 0 0 0 1.6-1.6v-2.2",
  "M1.3 8h3.9M10.8 8h3.9M3 6.1 1.1 8 3 9.9M13 6.1 14.9 8 13 9.9",
].join("");

/** The header of the handle column: ⟷ between two brackets. */
export function FitIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      <path
        d={FIT_PATH}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Edits a cell's text: Enter or leaving keeps it, Escape drops it. */
export function InlineEditor(props: {
  initial: string;
  placeholder?: string;
  onCommit: (text: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(props.initial);
  const [done, setDone] = useState(false);
  const finish = (keep: boolean) => {
    if (done) return;
    setDone(true);
    if (keep) props.onCommit(value);
    else props.onCancel();
  };
  return (
    <input
      className="st-input"
      autoFocus
      spellCheck={false}
      value={value}
      placeholder={props.placeholder}
      onFocus={(e) => e.currentTarget.select()}
      onChange={(e) => setValue(e.target.value)}
      onBlur={() => finish(true)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter" || e.key === "Tab") {
          e.preventDefault();
          finish(true);
        } else if (e.key === "Escape") {
          e.preventDefault();
          finish(false);
        }
      }}
    />
  );
}

export interface CellProps {
  text: CellText;
  popup?: boolean;
  center?: boolean;
  changed?: boolean;
  /** foreign_key: the → at the right. */
  arrow?: boolean;
  editing?: { initial: string; placeholder?: string } | null;
  onEdit?: () => void;
  onCommit?: (text: string) => void;
  onCancel?: () => void;
  /** Opens the popup menu (or the foreign key editor) over the cell. */
  onOpen?: (cell: HTMLElement) => void;
  /** A single click opens it too (foreign_key). */
  openOnClick?: boolean;
}

export function GridCell(p: CellProps) {
  const open = (e: React.MouseEvent) => {
    const cell = (e.currentTarget as HTMLElement).closest("td");
    if (cell) p.onOpen?.(cell);
  };
  return (
    <td
      className={cx(
        "st-cell",
        (p.popup || p.arrow) && "with-icon",
        p.center && "center",
        p.changed && "changed",
        p.text.dim && !p.editing && "dim",
      )}
      onClick={p.openOnClick ? open : undefined}
      onDoubleClick={p.openOnClick ? undefined : p.onEdit}
    >
      {p.editing ? (
        <InlineEditor
          initial={p.editing.initial}
          placeholder={p.editing.placeholder}
          onCommit={(t) => p.onCommit?.(t)}
          onCancel={() => p.onCancel?.()}
        />
      ) : (
        <span className="st-text">{p.text.text}</span>
      )}
      {p.popup && !p.editing && (
        <button
          className="st-icon"
          tabIndex={-1}
          aria-label="Choose"
          onMouseDown={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            open(e);
          }}
        >
          <Chevron />
        </button>
      )}
      {p.arrow && !p.editing && (
        <span className="st-icon arrow" aria-hidden>
          <Arrow />
        </span>
      )}
    </td>
  );
}
