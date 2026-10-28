// The connections page, laid out like TablePlus' welcome window: the app on the left,
// the toolbar, search and the tree of groups and connections on the right.

import { Database, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { EmptyState } from "@/components/StateView";
import { newConnection } from "@/domain/connection";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { LAYER, useActions } from "@/lib/actions";
import { openConnection } from "@/services/connect";
import { alertError, toastSuccess } from "@/services/errors";
import { HOME, useStore } from "@/store";
import { type ConnectionView, type Driver } from "@/types";
import { createGroup } from "./connectionActions";
import { connectionMenu, groupMenu, listMenu } from "./connectionMenus";
import { DriverPicker } from "./DriverPicker";
import { ConnectionForm } from "./form/ConnectionForm";
import { type FormInitial } from "./form/useConnectionForm";
import { ImportUrlDialog } from "./ImportUrlDialog";
import { importConnections } from "./transfer";
import { useHomeKeyboard } from "./useHomeKeyboard";
import { ConnectionTree } from "./welcome/ConnectionTree";
import { GroupDialog } from "./welcome/GroupDialog";
import { type ToolKind, ToolPicker } from "./welcome/ToolPicker";
import { buildTree, type SortBy } from "./welcome/tree";
import { WelcomeSide } from "./welcome/WelcomeSide";
import { WelcomeToolbar } from "./welcome/WelcomeToolbar";

/** Which dialog of the page is open. */
type Overlay =
  | { kind: "picker"; groupId: string | null }
  | { kind: "import"; url: string }
  | { kind: "form"; initial: FormInitial }
  | { kind: "tool"; tool: ToolKind }
  | null;

function useConnect() {
  const [connecting, setConnecting] = useState<string | null>(null);
  const connect = async (c: ConnectionView) => {
    setConnecting(c.id);
    try {
      await openConnection(c);
    } catch (e) {
      void alertError(`Couldn't connect to ${c.name}`, e, "connection-failed");
    } finally {
      setConnecting(null);
    }
  };
  return { connecting, connect };
}

function EmptyList(props: { query: string; onNewConnection: () => void }) {
  if (props.query) {
    return (
      <EmptyState
        compact
        art="no-results"
        icon={<Search size={18} />}
        title="No matching connections"
        hint={`Nothing matches “${props.query}”.`}
      />
    );
  }
  return (
    <EmptyState
      art="no-connections"
      icon={<Database size={20} />}
      title="No connections yet"
      hint="Connect to a database server, a cloud service or a local file."
      action={
        <button className="btn primary sm" onClick={props.onNewConnection}>
          New connection…
        </button>
      }
    />
  );
}

export function Home() {
  const connections = useStore((s) => s.connections);
  const groups = useStore((s) => s.groups);
  const restorable = useStore((s) => s.restorable);
  const visible = useStore((s) => s.view === HOME);
  const workspaces = useStore((s) => s.workspaces);
  const newConnectionRequest = useStore((s) => s.newConnectionRequest);
  const groupEditor = useStore((s) => s.groupEditor);
  const openIds = useMemo(() => Object.keys(workspaces), [workspaces]);
  const [query, setQuery] = useState("");
  const [sortBy, setSortBy] = useState<SortBy>("manual");
  const [selected, setSelected] = useState<string | null>(null);
  const [overlay, setOverlay] = useState<Overlay>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const menu = useContextMenu();
  const { connecting, connect } = useConnect();
  const rows = useMemo(
    () => buildTree(connections, groups, query, sortBy),
    [connections, groups, query, sortBy],
  );
  const listActive = visible && overlay === null && !groupEditor;

  const pickDriver = (groupId: string | null = null) => setOverlay({ kind: "picker", groupId });
  const importUrl = (url = "") => setOverlay({ kind: "import", url });
  const edit = (c: ConnectionView) =>
    setOverlay({ kind: "form", initial: { connection: c, view: c } });
  const close = () => setOverlay(null);
  const startForm = (driver: Driver, groupId: string | null) =>
    setOverlay({ kind: "form", initial: { connection: { ...newConnection(driver), groupId } } });

  useTauriEvent<string[]>("deep-link", (urls) => urls[0] && importUrl(urls[0]));
  useEffect(() => {
    if (newConnectionRequest) pickDriver();
  }, [newConnectionRequest]);

  useActions(LAYER.workspace, listActive, {
    "new-connection": () => pickDriver(),
    filter: () => searchRef.current?.focus(),
    "open-anything": () => searchRef.current?.focus(),
    refresh: () => void useStore.getState().loadConnections(),
  });
  useHomeKeyboard({
    active: listActive,
    rows,
    selected,
    searchRef,
    onSelect: setSelected,
    onConnect: connect,
    onClearSearch: () => setQuery(""),
  });

  const onSaved = async (v: ConnectionView, andConnect: boolean) => {
    close();
    await useStore.getState().loadConnections();
    setSelected(v.id);
    toastSuccess(`Saved "${v.name}".`);
    if (andConnect) void connect(v);
  };

  const restore = restorable.flatMap((id) => connections.filter((c) => c.id === id));
  const reopen = async () => {
    for (const c of restore) await connect(c);
  };
  const onListMenu = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest(".tree-row")) return;
    menu.open(
      e,
      listMenu({
        count: connections.length,
        sortBy,
        onSortBy: setSortBy,
        newConnection: () => pickDriver(),
        importUrl: () => importUrl(),
      }),
    );
  };

  return (
    <div className="home">
      <WelcomeSide
        restorable={restore}
        onReopen={() => void reopen()}
        onBackup={() => setOverlay({ kind: "tool", tool: "backup" })}
        onRestore={() => setOverlay({ kind: "tool", tool: "restore" })}
      />
      <section className="home-main">
        <WelcomeToolbar
          query={query}
          onQuery={setQuery}
          searchRef={searchRef}
          onNewConnection={() => pickDriver()}
          onNewGroup={() => createGroup()}
        />
        <div className="home-list" onContextMenu={onListMenu}>
          {rows.length === 0 ? (
            <EmptyList query={query} onNewConnection={() => pickDriver()} />
          ) : (
            <ConnectionTree
              rows={rows}
              openIds={openIds}
              selected={selected}
              connecting={connecting}
              menu={menu}
              onSelect={setSelected}
              onConnect={connect}
              connectionMenu={(c) =>
                connectionMenu(c, { isOpen: openIds.includes(c.id), groups, connect, edit })
              }
              groupMenu={(g) => groupMenu(g, { groups, newConnection: (id) => pickDriver(id) })}
            />
          )}
        </div>
      </section>
      {menu.node}
      {overlay?.kind === "picker" && (
        <DriverPicker
          onClose={close}
          onPick={(d) => startForm(d, overlay.groupId)}
          onImportUrl={() => importUrl()}
          onImportFile={() => {
            close();
            void importConnections();
          }}
          onImportTablePlus={() => {
            close();
            void importConnections("tableplus");
          }}
          onNewGroup={() => {
            close();
            createGroup(overlay.groupId);
          }}
        />
      )}
      {overlay?.kind === "import" && (
        <ImportUrlDialog
          initial={overlay.url}
          onClose={close}
          onDraft={(initial) => setOverlay({ kind: "form", initial })}
        />
      )}
      {overlay?.kind === "form" && (
        <ConnectionForm initial={overlay.initial} onClose={close} onSaved={onSaved} />
      )}
      {groupEditor && <GroupDialog editor={groupEditor} />}
      {overlay?.kind === "tool" && (
        <ToolPicker kind={overlay.tool} connections={connections} onClose={close} />
      )}
    </div>
  );
}
