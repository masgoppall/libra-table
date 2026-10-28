// The workspace toolbar, laid out like TablePlus' main window: grouped buttons around the
// connection's status pill ("LOCAL | name : server : database").

import { Check } from "lucide-react";
import { Fragment, type ReactNode, useCallback, useState } from "react";
import { type MenuItem, useContextMenu } from "@/components/ContextMenu";
import { DRIVER_LABEL, isProduction } from "@/domain/connection";
import { SAFE_MODES } from "@/domain/safeMode";
import { cx } from "@/lib/cx";
import { type ActionId, dispatch, withShortcut } from "@/lib/actions";
import { setSafeMode } from "@/services/connectionSettings";
import { activeTab, HOME, useConnection, useStore } from "@/store";
import { type ConnectionView, type Workspace } from "@/types";
import { ConnectionPopover } from "./ConnectionPopover";
import * as icon from "./icons";

interface ToolButton {
  label: string;
  glyph: ReactNode;
  onClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
  disabled?: boolean;
  /** Keyboard shortcut shown in the tooltip. */
  action?: ActionId;
  active?: boolean;
}

/** A capsule of buttons; `separated` groups draw a line between them and space them wider. */
function Group(props: { buttons: ToolButton[]; separated?: "wide" | "narrow" }) {
  return (
    <div className={cx("tb-group", props.separated && `separated ${props.separated}`)}>
      {props.buttons.map((b, i) => (
        <Fragment key={b.label}>
          {props.separated && i > 0 && <span className="tb-sep" />}
          <button
            className={cx("tb-btn", b.active && "active")}
            disabled={b.disabled}
            onClick={b.onClick}
            title={b.action ? withShortcut(b.label, b.action) : b.label}
            aria-label={b.label}
            aria-pressed={b.active}
          >
            {b.glyph}
          </button>
        </Fragment>
      ))}
    </div>
  );
}

/** The server as TablePlus names it: "MySQL 8.0.40". */
function serverLabel(ws: Workspace): string {
  const label = DRIVER_LABEL[ws.info.driver];
  const version = ws.info.serverVersion.trim();
  if (!version) return label;
  return version.toLowerCase().startsWith(label.toLowerCase()) ? version : `${label} ${version}`;
}

function statusText(ws: Workspace, conn: ConnectionView | undefined): string {
  const parts = [
    conn?.name,
    ws.info.throughTunnel ? "SSH" : null,
    serverLabel(ws),
    ws.database || null,
  ].filter(Boolean);
  return `${(conn?.tag ?? "local").toUpperCase()} | ${parts.join(" : ")}`;
}

function StatusPill({ ws }: { ws: Workspace | null }) {
  const conn = useConnection(ws?.connectionId ?? "");
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  if (!ws || !conn) {
    return (
      <div className="tb-status idle" data-tauri-drag-region>
        <span data-tauri-drag-region>Connections</span>
      </div>
    );
  }
  const text = statusText(ws, conn);
  const style = conn.color ? ({ "--status": conn.color } as React.CSSProperties) : undefined;
  return (
    <div className="tb-status-wrap">
      <button
        className={cx("tb-status", `tag-${conn.tag}`, conn.color && "custom", open && "open")}
        style={style}
        title={text}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="tb-status-text">{text}</span>
        {conn.options.readOnly && <span className="tb-status-badge read-only">read-only</span>}
      </button>
      {open && <ConnectionPopover ws={ws} conn={conn} onClose={close} />}
    </div>
  );
}

function safeModeMenu(conn: ConnectionView): MenuItem[] {
  return SAFE_MODES.map((m) => ({
    label: m.label,
    icon: conn.options.safeMode === m.id ? <Check size={14} /> : <span className="icon-gap" />,
    onClick: () => void setSafeMode(conn, m.id),
  }));
}

export function Toolbar() {
  const view = useStore((s) => s.view);
  const ws = useStore((s) => (s.view === HOME ? null : (s.workspaces[s.view] ?? null)));
  const consoleOpen = useStore((s) => s.consoleOpen);
  const tab = ws ? activeTab(ws) : undefined;
  const dirty = useStore((s) => (tab ? (s.pendingEdits[tab.id] ?? 0) : 0));
  const conn = useConnection(ws?.connectionId ?? "");
  const menu = useContextMenu();
  const run = (a: ActionId) => () => void dispatch(a);
  const caps = ws?.caps;
  const safe = conn && conn.options.safeMode !== "auto" && conn.options.safeMode !== "silent";

  return (
    <div className={cx("toolbar-bar", isProduction(conn) && "production")} data-tauri-drag-region>
      <div className="tb-traffic" data-tauri-drag-region />
      <Group
        buttons={[
          {
            label: "Toggle sidebar",
            glyph: <icon.SidebarIcon />,
            onClick: run("toggle-sidebar"),
            disabled: !ws,
            action: "toggle-sidebar",
          },
          {
            label: "Connections",
            glyph: <icon.PlugIcon />,
            onClick: run("home"),
            active: view === HOME,
            action: "home",
          },
        ]}
      />
      <Group
        separated="wide"
        buttons={[
          {
            label: "Discard changes",
            glyph: <icon.DiscardIcon />,
            onClick: run("discard"),
            disabled: !dirty,
          },
          {
            label: "Preview changes",
            glyph: <icon.PreviewIcon />,
            onClick: run("save"),
            disabled: !dirty,
          },
          {
            label: "Commit changes",
            glyph: <icon.CommitIcon />,
            onClick: run("save"),
            disabled: !dirty,
            action: "save",
          },
        ]}
      />
      <Group
        buttons={[
          {
            label: "Safe mode",
            glyph: <icon.LockIcon />,
            onClick: (e) => conn && menu.openAt(e.currentTarget, safeModeMenu(conn)),
            disabled: !conn || !ws,
            active: !!safe,
          },
          {
            label: "Open database",
            glyph: <icon.DatabaseIcon />,
            onClick: run("switch-database"),
            disabled: !caps?.switchDatabase,
            action: "switch-database",
          },
          {
            label: "New query",
            glyph: <span className="tb-sql">SQL</span>,
            onClick: run("new-query"),
            disabled: !caps?.query,
            action: "new-query",
          },
        ]}
      />
      <StatusPill ws={ws} />
      <Group
        buttons={[
          {
            label: "Refresh",
            glyph: <icon.RefreshIcon />,
            onClick: run("refresh"),
            disabled: !ws,
            action: "refresh",
          },
          {
            label: caps?.metrics ? "Metrics" : "Process list",
            glyph: <icon.MetricsIcon />,
            onClick: run(caps?.metrics ? "open-metrics" : "process-list"),
            disabled: !caps?.metrics && !caps?.processes,
          },
          {
            label: "Open anything",
            glyph: <icon.CommandIcon />,
            onClick: run("open-anything"),
            disabled: !ws,
            action: "open-anything",
          },
        ]}
      />
      <Group
        separated="narrow"
        buttons={[
          {
            label: "Console log",
            glyph: <icon.ConsoleIcon />,
            onClick: run("toggle-console"),
            active: consoleOpen,
            action: "toggle-console",
          },
          {
            label: "Row detail",
            glyph: <icon.DetailIcon />,
            onClick: run("toggle-detail"),
            disabled: tab?.kind !== "data",
            action: "toggle-detail",
          },
        ]}
      />
      {menu.node}
    </div>
  );
}
