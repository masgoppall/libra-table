// The row above the structure grid: the table's Name (typed to rename it), its Primary
// key columns as tokens (click to choose), and "Search for column…".

import { type ContextMenuApi } from "@/components/ContextMenu";
import { editColumn, live } from "./designerModel";
import { type TableDesignerState } from "./useTableDesigner";

function SearchIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 15 15" aria-hidden>
      <circle cx="6.2" cy="6.2" r="5" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="m9.9 9.9 4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

export function StructureHeader(p: {
  d: TableDesignerState;
  menu: ContextMenuApi;
  readOnly: string | null;
  search: string;
  onSearch: (q: string) => void;
}) {
  const { d } = p;
  const cols = live(d.cols);
  const keys = cols.filter((r) => r.cur.primaryKey);

  const choosePrimary = (el: HTMLElement) => {
    if (p.readOnly) return;
    const items = cols
      .filter((r) => r.cur.name.trim())
      .map((r) => ({
        label: r.cur.name.trim(),
        checked: r.cur.primaryKey,
        onClick: () =>
          d.setCols((rs) =>
            rs.map((x) =>
              x.key === r.key
                ? { ...x, cur: editColumn(x.cur, { primaryKey: !x.cur.primaryKey }) }
                : x,
            ),
          ),
      }));
    p.menu.openAt(el, items.length ? items : [{ label: "No columns yet", disabled: true }]);
  };

  return (
    <div className="st-head">
      <label className="st-field-label" htmlFor="st-name">
        Name
      </label>
      <input
        id="st-name"
        className="st-name"
        spellCheck={false}
        placeholder="table_name"
        value={d.name}
        readOnly={!!p.readOnly}
        autoFocus={d.creating}
        onChange={(e) => d.setName(e.target.value)}
      />
      <span className="st-field-label st-primary-label">Primary</span>
      <button
        className="st-primary"
        aria-label="Primary key"
        disabled={!!p.readOnly}
        onClick={(e) => choosePrimary(e.currentTarget)}
      >
        {keys.map((r) => (
          <span key={r.key} className="st-token">
            {r.cur.name.trim() || "?"}
          </span>
        ))}
      </button>
      <label className="st-search">
        <SearchIcon />
        <input
          spellCheck={false}
          placeholder="Search for column..."
          value={p.search}
          onChange={(e) => p.onSearch(e.target.value)}
        />
      </label>
    </div>
  );
}
