import { Download, FolderPlus, Link2, MoreHorizontal, Plus, Search, Upload } from "lucide-react";
import { type RefObject } from "react";
import { Kbd } from "@/components/Kbd";
import { shortcutFor, withShortcut } from "@/lib/actions";
import { useContextMenu } from "@/components/ContextMenu";
import { createGroup } from "./connectionActions";
import { exportConnections, importConnections } from "./transfer";
import { type SortBy } from "./useConnectionSections";

export interface HomeHeaderProps {
  count: number;
  query: string;
  onQuery: (q: string) => void;
  searchRef: RefObject<HTMLInputElement>;
  sortBy: SortBy;
  onSortBy: (s: SortBy) => void;
  onImportUrl: () => void;
  onNewConnection: () => void;
}

export function HomeHeader(p: HomeHeaderProps) {
  const menu = useContextMenu();
  const more = (el: HTMLElement) =>
    menu.openAt(el, [
      {
        label: "Import connections…",
        icon: <Download size={14} />,
        onClick: () => void importConnections(),
      },
      {
        label: "Export all connections…",
        icon: <Upload size={14} />,
        disabled: p.count === 0,
        onClick: () => void exportConnections([]),
      },
    ]);
  return (
    <div className="home-header">
      <div className="home-title">
        <h1>Connections</h1>
        {p.count > 0 && <span className="home-count">{p.count}</span>}
      </div>
      <div className="home-search">
        <Search size={14} className="faint" />
        <input
          ref={p.searchRef}
          placeholder="Search connections"
          value={p.query}
          onChange={(e) => p.onQuery(e.target.value)}
        />
        {!p.query && <Kbd>{shortcutFor("filter")}</Kbd>}
      </div>
      <select
        className="select"
        value={p.sortBy}
        onChange={(e) => p.onSortBy(e.target.value as SortBy)}
        title="Sort by"
      >
        <option value="name">Sort by name</option>
        <option value="driver">Sort by driver</option>
        <option value="tag">Sort by tag</option>
      </select>
      <span className="spacer" />
      <button className="btn" onClick={createGroup}>
        <FolderPlus size={14} /> New group
      </button>
      <button className="btn" onClick={p.onImportUrl}>
        <Link2 size={14} /> Import URL
      </button>
      <button
        className="btn primary"
        onClick={p.onNewConnection}
        title={withShortcut("New connection", "new-connection")}
      >
        <Plus size={14} /> New connection
      </button>
      <button
        className="btn ghost icon"
        onClick={(e) => more(e.currentTarget)}
        title="Import or export connections"
        aria-label="More"
      >
        <MoreHorizontal size={15} />
      </button>
      {menu.node}
    </div>
  );
}
