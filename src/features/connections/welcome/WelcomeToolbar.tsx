// The welcome window's toolbar: new connection and new group, and the search field.

import { Search } from "lucide-react";
import { type RefObject } from "react";
import { withShortcut } from "@/lib/actions";
import { FolderPlusIcon, PlusIcon } from "./icons";

export function WelcomeToolbar(props: {
  query: string;
  onQuery: (q: string) => void;
  searchRef: RefObject<HTMLInputElement>;
  onNewConnection: () => void;
  onNewGroup: () => void;
}) {
  return (
    <div className="home-toolbar" data-tauri-drag-region>
      <div className="home-pill home-actions">
        <button
          className="home-pill-btn"
          onClick={props.onNewConnection}
          title={withShortcut("New connection", "new-connection")}
          aria-label="New connection"
        >
          <PlusIcon size={13.5} />
        </button>
        <button
          className="home-pill-btn"
          onClick={props.onNewGroup}
          title="New group"
          aria-label="New group"
        >
          <FolderPlusIcon />
        </button>
      </div>
      <label className="home-pill home-search">
        <Search size={15} strokeWidth={2.1} aria-hidden />
        <input
          ref={props.searchRef}
          placeholder="Search for connection..."
          value={props.query}
          spellCheck={false}
          onChange={(e) => props.onQuery(e.target.value)}
        />
      </label>
    </div>
  );
}
