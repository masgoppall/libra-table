import {
  Columns2,
  Columns3,
  Download,
  Filter,
  PanelRight,
  Plus,
  RefreshCw,
  Save,
  Trash2,
  Undo2,
} from "lucide-react";
import { BusyIcon } from "@/components/Spinner";
import { withShortcut } from "@/lib/actions";
import { cx } from "@/lib/cx";
import { pluralWord } from "@/lib/format";

export interface DataToolbarProps {
  filtersOpen: boolean;
  activeFilters: number;
  loading: boolean;
  editable: boolean;
  /** Whether the page loaded (the read-only note waits for it). */
  loaded: boolean;
  /** Why rows can't be edited, shown when `editable` is false. */
  readOnlyNote: string;
  hasSelection: boolean;
  dirty: number;
  detailOpen: boolean;
  onToggleFilters: () => void;
  onReload: () => void;
  onAddRow: () => void;
  onDelete: () => void;
  onDiscard: () => void;
  onCommit: () => void;
  onExport: () => void;
  onStructure: () => void;
  onToggleDetail: () => void;
  /** How many columns are hidden (shown on the Columns button). */
  hiddenCount: number;
  onColumns: (anchor: HTMLElement) => void;
}

function PendingGroup(p: DataToolbarProps) {
  return (
    <>
      <div className="tool-sep" />
      <div className="tool-group pending-group">
        <span className="pending-label">
          {p.dirty} pending {pluralWord(p.dirty, "change")}
        </span>
        <button className="btn sm" onClick={p.onDiscard} title="Discard all edits">
          <Undo2 size={13} /> Discard
        </button>
        <button
          className="btn sm primary"
          onClick={p.onCommit}
          title={withShortcut("Review and commit", "save")}
        >
          <Save size={13} /> Commit
        </button>
      </div>
    </>
  );
}

export function DataToolbar(p: DataToolbarProps) {
  const filterClass = cx("btn sm", p.filtersOpen && "active", p.activeFilters > 0 && "has-badge");
  const addTitle = p.editable
    ? withShortcut("Add row", "add-row")
    : "Read-only: the table has no primary key";
  return (
    <div className="toolbar">
      <div className="tool-group">
        <button
          className={filterClass}
          onClick={p.onToggleFilters}
          title={withShortcut("Filter rows", "filter")}
        >
          <Filter size={13} /> Filter
          {p.activeFilters > 0 && <span className="btn-badge">{p.activeFilters}</span>}
        </button>
        <button
          className="btn sm icon"
          title={withShortcut("Refresh", "refresh")}
          onClick={p.onReload}
        >
          <BusyIcon busy={p.loading} icon={<RefreshCw size={13} />} />
        </button>
      </div>
      <div className="tool-sep" />
      <div className="tool-group">
        <button className="btn sm" disabled={!p.editable} title={addTitle} onClick={p.onAddRow}>
          <Plus size={13} /> Row
        </button>
        <button
          className="btn sm"
          disabled={!p.editable || !p.hasSelection}
          onClick={p.onDelete}
          title="Delete selected rows (⌫)"
        >
          <Trash2 size={13} /> Delete
        </button>
      </div>
      {p.dirty > 0 && <PendingGroup {...p} />}
      {!p.editable && p.loaded && <span className="readonly-note">{p.readOnlyNote}</span>}
      <span className="spacer" />
      <div className="tool-group">
        <button
          className={cx("btn sm", p.hiddenCount > 0 && "has-badge")}
          onClick={(e) => p.onColumns(e.currentTarget)}
          title="Show or hide columns"
        >
          <Columns2 size={13} /> Columns
          {p.hiddenCount > 0 && <span className="btn-badge">{p.hiddenCount} hidden</span>}
        </button>
        <button className="btn sm" onClick={p.onExport} title={withShortcut("Export", "export")}>
          <Download size={13} /> Export
        </button>
        <button
          className="btn sm"
          title={withShortcut("Structure", "open-structure")}
          onClick={p.onStructure}
        >
          <Columns3 size={13} /> Structure
        </button>
        <button
          className={`btn sm icon${p.detailOpen ? " active" : ""}`}
          title={withShortcut("Row detail", "toggle-detail")}
          onClick={p.onToggleDetail}
        >
          <PanelRight size={13} />
        </button>
      </div>
    </div>
  );
}
