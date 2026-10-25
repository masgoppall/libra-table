// ⌘P "open anything" (tables, views, saved queries, actions) and ⌘K "switch database".

import { Database, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Highlight } from "@/components/Highlight";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions } from "@/lib/actions";
import { type SearchResult, search } from "@/lib/fuzzy";
import { type PaletteKind, store, useStore } from "@/store";
import { type Workspace } from "@/types";
import { anythingItems, databaseItems, type PaletteItem } from "./paletteItems";

const TEXT: Record<PaletteKind, { label: string; placeholder: string }> = {
  anything: {
    label: "Open anything",
    placeholder: "Open a table, view or saved query, or run an action…",
  },
  database: { label: "Switch database", placeholder: "Switch to database…" },
};

function useItems(kind: PaletteKind, ws: Workspace) {
  const [items, setItems] = useState<PaletteItem[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    const load = kind === "database" ? databaseItems : anythingItems;
    void load(ws).then((list) => !cancelled && setItems(list));
    return () => {
      cancelled = true;
    };
  }, [kind, ws.connectionId]);
  return items;
}

function Row(props: {
  result: SearchResult<PaletteItem>;
  head: string | null;
  active: boolean;
  query: string;
  onHover: () => void;
  onRun: (alt: boolean) => void;
}) {
  const { item, match } = props.result;
  const detail = props.query ? item.group : item.detail;
  return (
    <div>
      {props.head && <div className="palette-group">{props.head}</div>}
      <div
        className={`palette-item${props.active ? " active" : ""}`}
        onMouseMove={() => !props.active && props.onHover()}
        onClick={(e) => props.onRun(e.shiftKey)}
      >
        <span className="palette-icon">{item.icon}</span>
        <span className="palette-label">
          <Highlight text={item.label} indexes={match.indexes} />
        </span>
        {detail && <span className="palette-detail">{detail}</span>}
        {item.hint && props.active && <span className="palette-hint">{item.hint}</span>}
      </div>
    </div>
  );
}

function Footer({ kind }: { kind: PaletteKind }) {
  return (
    <div className="palette-foot">
      <span>
        <kbd>↑</kbd>
        <kbd>↓</kbd> move
      </span>
      <span>
        <kbd>↵</kbd> open
      </span>
      <span>
        {kind === "anything" ? (
          <>
            <kbd>⌘K</kbd> databases
          </>
        ) : (
          <>
            <kbd>⌘P</kbd> tables
          </>
        )}
      </span>
    </div>
  );
}

function PaletteBody({ kind, ws }: { kind: PaletteKind; ws: Workspace }) {
  const id = ws.connectionId;
  const { closePalette, openPalette } = store.get();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const items = useItems(kind, ws);

  // ⌘P / ⌘K again closes; the other key switches palettes.
  const toggle = (k: PaletteKind) => (kind === k ? closePalette() : openPalette(k, id));
  useActions(LAYER.dialog, true, {
    "close-tab": closePalette,
    "open-anything": () => toggle("anything"),
    "switch-database": () => toggle("database"),
  });

  const matches = useMemo(
    () => search(items ?? [], query, "fuzzy", (i) => i.label),
    [items, query],
  );
  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector(".palette-item.active")?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const run = (i: number, alt: boolean) => {
    const m = matches[i];
    if (!m) return;
    closePalette();
    // Let the palette unmount before the action opens its own dialog.
    setTimeout(() => m.item.run(alt), 0);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const moves: Record<string, () => void> = {
      ArrowDown: () => setActive((a) => Math.min(matches.length - 1, a + 1)),
      ArrowUp: () => setActive((a) => Math.max(0, a - 1)),
      Enter: () => run(active, e.shiftKey),
      Escape: () => {
        e.stopPropagation();
        closePalette();
      },
    };
    const fn = moves[e.key];
    if (!fn) return;
    e.preventDefault();
    fn();
  };

  return (
    <div
      className="palette-overlay"
      onMouseDown={(e) => e.target === e.currentTarget && closePalette()}
    >
      <div className="palette" role="dialog" aria-label={TEXT[kind].label}>
        <div className="palette-input">
          {kind === "database" ? (
            <Database size={16} className="faint" />
          ) : (
            <Search size={16} className="faint" />
          )}
          <input
            autoFocus
            value={query}
            placeholder={TEXT[kind].placeholder}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
          />
          <kbd>esc</kbd>
        </div>
        <div className="palette-list" ref={listRef}>
          {items === null && (
            <div className="palette-empty">
              <Spinner /> Loading…
            </div>
          )}
          {items !== null && matches.length === 0 && (
            <div className="palette-empty">Nothing matches “{query}”.</div>
          )}
          {matches.map((m, i) => {
            const head =
              !query && m.item.group !== matches[i - 1]?.item.group ? m.item.group : null;
            return (
              <Row
                key={m.item.key}
                result={m}
                head={head}
                active={i === active}
                query={query}
                onHover={() => setActive(i)}
                onRun={(alt) => run(i, alt)}
              />
            );
          })}
        </div>
        <Footer kind={kind} />
      </div>
    </div>
  );
}

export function Palette() {
  const palette = useStore((s) => s.palette);
  const ws = useStore((s) => (s.palette ? s.workspaces[s.palette.connectionId] : undefined));
  if (!palette || !ws) return null;
  return <PaletteBody key={`${palette.kind}-${ws.connectionId}`} kind={palette.kind} ws={ws} />;
}
