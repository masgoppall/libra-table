import { MoreHorizontal } from "lucide-react";
import { DriverLogo } from "@/components/DriverLogo";
import { Spinner } from "@/components/Spinner";
import { connectionColor, DRIVER_LABEL } from "@/domain/connection";
import { type ConnectionView } from "@/types";

export interface ConnectionRowProps {
  conn: ConnectionView;
  selected: boolean;
  isOpen: boolean;
  connecting: boolean;
  onSelect: () => void;
  onConnect: () => void;
  onMenu: (e: React.MouseEvent) => void;
  onMenuButton: (el: HTMLElement) => void;
}

function Status({ conn, isOpen }: { conn: ConnectionView; isOpen: boolean }) {
  if (!isOpen) return <span className="faint">{DRIVER_LABEL[conn.driver]}</span>;
  return (
    <span className="conn-open">
      <span className="live-dot" /> Open
    </span>
  );
}

export function ConnectionRow(p: ConnectionRowProps) {
  const c = p.conn;
  return (
    <div
      data-conn={c.id}
      className={`conn-row${p.selected ? " selected" : ""}`}
      onClick={p.onSelect}
      onDoubleClick={p.onConnect}
      onContextMenu={p.onMenu}
    >
      <span className="conn-stripe" style={{ background: connectionColor(c) }} />
      <DriverLogo driver={c.driver} size={32} />
      <div className="conn-main">
        <div className="conn-name">
          <span className="title">{c.name}</span>
          <span className={`badge ${c.tag}`}>{c.tag}</span>
        </div>
        <div className="conn-summary">
          {c.ssh.enabled && <span className="ssh-chip">SSH {c.ssh.host}</span>}
          {c.summary}
        </div>
      </div>
      <div className="conn-meta">
        <Status conn={c} isOpen={p.isOpen} />
      </div>
      <div className="conn-actions">
        {p.connecting ? (
          <Spinner size={15} className="muted" />
        ) : (
          <>
            <button className="btn sm conn-connect" onClick={p.onConnect}>
              {p.isOpen ? "Show" : "Connect"}
            </button>
            <button
              className="btn ghost icon sm"
              onClick={(e) => p.onMenuButton(e.currentTarget)}
              aria-label="Actions"
            >
              <MoreHorizontal size={15} />
            </button>
          </>
        )}
      </div>
    </div>
  );
}
