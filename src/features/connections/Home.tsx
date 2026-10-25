// The connections page: list, search, groups, driver picker, URL import and the form.

import { useEffect, useMemo, useRef, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { Kbd } from "@/components/Kbd";
import { newConnection } from "@/domain/connection";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { LAYER, shortcutFor, useActions } from "@/lib/actions";
import { openConnection } from "@/services/connect";
import { showError, toastSuccess } from "@/services/errors";
import { HOME, useStore } from "@/store";
import { type ConnectionView } from "@/types";
import { ConnectionList } from "./ConnectionList";
import { DriverPicker } from "./DriverPicker";
import { ConnectionForm } from "./form/ConnectionForm";
import { type FormInitial } from "./form/useConnectionForm";
import { HomeHeader } from "./HomeHeader";
import { ImportUrlDialog } from "./ImportUrlDialog";
import { RestoreBar } from "./RestoreBar";
import { type SortBy, useConnectionSections } from "./useConnectionSections";
import { useHomeKeyboard } from "./useHomeKeyboard";
import { WelcomeHero } from "./WelcomeHero";

/** Which dialog of the page is open. */
type Overlay =
  | { kind: "picker" }
  | { kind: "import"; url: string }
  | { kind: "form"; initial: FormInitial }
  | null;

function useConnect() {
  const [connecting, setConnecting] = useState<string | null>(null);
  const connect = async (c: ConnectionView) => {
    setConnecting(c.id);
    try {
      await openConnection(c);
    } catch (e) {
      showError(e);
    } finally {
      setConnecting(null);
    }
  };
  return { connecting, connect };
}

function KeyboardHint() {
  return (
    <div className="home-foot faint small">
      <Kbd>↑</Kbd>
      <Kbd>↓</Kbd> select · <Kbd>↵</Kbd> connect · right-click for more ·{" "}
      <Kbd>{shortcutFor("shortcuts")}</Kbd> shortcuts
    </div>
  );
}

export function Home() {
  const connections = useStore((s) => s.connections);
  const groups = useStore((s) => s.groups);
  const restorable = useStore((s) => s.restorable);
  const visible = useStore((s) => s.view === HOME);
  const workspaces = useStore((s) => s.workspaces);
  const newConnectionRequest = useStore((s) => s.newConnectionRequest);
  const openIds = useMemo(() => Object.keys(workspaces), [workspaces]);
  const [query, setQuery] = useState("");
  const [sortBy, setSortBy] = useState<SortBy>("name");
  const [selected, setSelected] = useState<string | null>(null);
  const [overlay, setOverlay] = useState<Overlay>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const menu = useContextMenu();
  const { connecting, connect } = useConnect();
  const { sections, visible: list } = useConnectionSections(connections, groups, query, sortBy);
  const listActive = visible && overlay === null;

  const pickDriver = () => setOverlay({ kind: "picker" });
  const importUrl = (url = "") => setOverlay({ kind: "import", url });
  const edit = (c: ConnectionView) =>
    setOverlay({ kind: "form", initial: { connection: c, view: c } });
  const close = () => setOverlay(null);

  useTauriEvent<string[]>("deep-link", (urls) => urls[0] && importUrl(urls[0]));
  useEffect(() => {
    if (newConnectionRequest) pickDriver();
  }, [newConnectionRequest]);

  useActions(LAYER.workspace, listActive, {
    "new-connection": pickDriver,
    filter: () => searchRef.current?.focus(),
    "open-anything": () => searchRef.current?.focus(),
    refresh: () => void useStore.getState().loadConnections(),
  });
  useHomeKeyboard({
    active: listActive,
    list,
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

  return (
    <div className="home">
      <HomeHeader
        count={connections.length}
        query={query}
        onQuery={setQuery}
        searchRef={searchRef}
        sortBy={sortBy}
        onSortBy={setSortBy}
        onImportUrl={() => importUrl()}
        onNewConnection={pickDriver}
      />
      <div className="home-body">
        <RestoreBar connections={restore} connecting={connecting} onConnect={connect} />
        {connections.length === 0 ? (
          <WelcomeHero onNewConnection={pickDriver} onImportUrl={() => importUrl()} />
        ) : list.length === 0 && query ? (
          <div className="faint pad-y center">No connections match “{query}”.</div>
        ) : (
          <ConnectionList
            sections={sections}
            groups={groups}
            query={query}
            openIds={openIds}
            selected={selected}
            connecting={connecting}
            menu={menu}
            onSelect={setSelected}
            onConnect={connect}
            onEdit={edit}
          />
        )}
        {connections.length > 0 && <KeyboardHint />}
      </div>
      {menu.node}
      {overlay?.kind === "picker" && (
        <DriverPicker
          onClose={close}
          onPick={(d) => setOverlay({ kind: "form", initial: { connection: newConnection(d) } })}
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
    </div>
  );
}
