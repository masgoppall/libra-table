// CON-3: choose the kind of database for a new connection — TablePlus' grid of round
// badges in a sheet, with import and "New Group" next to Create.

import { useMemo, useRef, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { DriverCircle } from "@/components/DriverLogo";
import { Sheet, SheetSearch } from "@/components/Sheet";
import { useStore } from "@/store";
import { type Driver, type DriverEntry } from "@/types";

const COLUMNS = 6;

/** TablePlus' order of the grid. */
const ORDER: Driver[] = [
  "postgres",
  "redshift",
  "mysql",
  "mariadb",
  "mssql",
  "cassandra",
  "clickhouse",
  "bigquery",
  "dynamodb",
  "elasticsearch",
  "libsql",
  "cloudflare_d1",
  "mongodb",
  "snowflake",
  "redis",
  "sqlite",
  "duckdb",
  "oracle",
  "cockroach",
  "greenplum",
  "vertica",
];

const rank = (d: Driver) => {
  const i = ORDER.indexOf(d);
  return i < 0 ? ORDER.length : i;
};

function matches(d: DriverEntry, query: string) {
  const q = query.trim().toLowerCase();
  return !q || d.label.toLowerCase().includes(q) || d.id.includes(q);
}

/** The next available driver from `from` going `step` cells (wrapping around). */
function move(list: DriverEntry[], from: number, step: number): number {
  for (let k = 1; k <= list.length; k++) {
    const i = from + step * k;
    if (i < 0 || i >= list.length) return from;
    if (list[i]?.ready) return i;
    if (Math.abs(step) === 1) continue;
    return from;
  }
  return from;
}

function DriverTile(props: {
  d: DriverEntry;
  selected: boolean;
  onSelect: () => void;
  onPick: () => void;
}) {
  const { d } = props;
  return (
    <button
      type="button"
      className={`picker-tile${props.selected ? " selected" : ""}`}
      disabled={!d.ready}
      aria-pressed={props.selected}
      onClick={props.onSelect}
      onDoubleClick={props.onPick}
      title={d.ready ? d.note || d.label : `${d.label} is not available yet`}
    >
      <DriverCircle driver={d.id} size={40} />
      <span className="picker-name">{d.label}</span>
    </button>
  );
}

export interface DriverPickerProps {
  onPick: (d: Driver) => void;
  onClose: () => void;
  onImportUrl: () => void;
  onImportFile: () => void;
  onImportTablePlus: () => void;
  onNewGroup: () => void;
}

export function DriverPicker(props: DriverPickerProps) {
  const drivers = useStore((s) => s.drivers);
  const [selected, setSelected] = useState<Driver>("postgres");
  const [query, setQuery] = useState("");
  const importRef = useRef<HTMLButtonElement>(null);
  const menu = useContextMenu();
  const shown = useMemo(
    () => drivers.filter((d) => matches(d, query)).sort((a, b) => rank(a.id) - rank(b.id)),
    [drivers, query],
  );
  const current = shown.find((d) => d.id === selected && d.ready) ?? shown.find((d) => d.ready);
  const pick = (id: Driver | undefined) => {
    if (id && drivers.find((d) => d.id === id)?.ready) props.onPick(id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const steps: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: COLUMNS,
      ArrowUp: -COLUMNS,
    };
    if (e.key === "Enter") {
      e.preventDefault();
      pick(current?.id);
      return;
    }
    const step = steps[e.key];
    if (step === undefined || !current) return;
    // Left / right move the caret while there is text to edit.
    if (Math.abs(step) === 1 && query) return;
    e.preventDefault();
    const at = shown.indexOf(current);
    setSelected(shown[move(shown, at, step)]?.id ?? current.id);
  };

  const importMenu = () => {
    if (!importRef.current) return;
    menu.openAt(importRef.current, [
      { label: "From URL…", onClick: props.onImportUrl },
      { label: "From file…", onClick: props.onImportFile },
      { label: "From TablePlus…", onClick: props.onImportTablePlus },
    ]);
  };

  return (
    <Sheet
      label="New connection"
      className="picker-sheet"
      onClose={props.onClose}
      footer={
        <>
          <button className="sheet-btn" onClick={props.onClose}>
            Cancel
          </button>
          <span className="spacer" />
          <button ref={importRef} className="sheet-btn wide" onClick={importMenu}>
            Import Connection
          </button>
          <button className="sheet-btn" onClick={props.onNewGroup}>
            New Group
          </button>
          <button
            className="sheet-btn primary"
            disabled={!current}
            onClick={() => pick(current?.id)}
          >
            Create
          </button>
        </>
      }
    >
      <div className="picker-body" onKeyDown={onKeyDown}>
        <SheetSearch value={query} onChange={setQuery} placeholder="Search" />
        <div className="picker-grid" role="group" aria-label="Database drivers">
          {shown.map((d) => (
            <DriverTile
              key={d.id}
              d={d}
              selected={current?.id === d.id}
              onSelect={() => setSelected(d.id)}
              onPick={() => pick(d.id)}
            />
          ))}
          {shown.length === 0 && <div className="picker-empty">No driver matches “{query}”.</div>}
        </div>
      </div>
      {menu.node}
    </Sheet>
  );
}
