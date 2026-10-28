// A click on the status pill: what this window is connected to (TablePlus'
// ConnectionInfoPopover), with the connection URL to copy.

import { useEffect, useRef } from "react";
import { DriverCircle } from "@/components/DriverLogo";
import { DRIVER_LABEL } from "@/domain/connection";
import { SAFE_MODES } from "@/domain/safeMode";
import { copyConnectionUrl } from "@/features/connections/connectionActions";
import { type ConnectionView, type Workspace } from "@/types";

function Row({ label, value }: { label: string; value: string | null | undefined }) {
  if (!value) return null;
  return (
    <>
      <span className="cip-label">{label}</span>
      <span className="cip-value selectable" title={value}>
        {value}
      </span>
    </>
  );
}

export function ConnectionPopover(props: {
  ws: Workspace;
  conn: ConnectionView;
  onClose: () => void;
}) {
  const { ws, conn } = props;
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const pill = (e.target as HTMLElement).closest(".tb-status");
      if (!ref.current?.contains(e.target as Node) && !pill) props.onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [props.onClose]);
  const safe = SAFE_MODES.find((m) => m.id === conn.options.safeMode)?.label;
  const server = `${DRIVER_LABEL[ws.info.driver]} ${ws.info.serverVersion}`.trim();
  return (
    <div className="cip w-tokens" ref={ref} role="dialog" aria-label="Connection info">
      <div className="cip-head">
        <DriverCircle driver={conn.driver} size={32} />
        <div className="cip-titles">
          <div className="cip-name">{conn.name}</div>
          <div className="cip-server">{server}</div>
        </div>
        <span className={`tag-badge ${conn.tag}`}>{conn.tag}</span>
      </div>
      <div className="cip-grid">
        <Row label="Address" value={conn.summary} />
        <Row label="User" value={conn.db.user} />
        <Row label="Database" value={ws.database || "none selected"} />
        <Row label="Schema" value={ws.schema && ws.schema !== ws.database ? ws.schema : null} />
        <Row label="SSH" value={ws.info.throughTunnel ? `via ${conn.ssh.host}` : null} />
        <Row label="Safe mode" value={safe} />
        <Row label="Access" value={conn.options.readOnly ? "Read-only" : "Read & write"} />
      </div>
      <div className="cip-foot">
        <button
          className="sheet-btn"
          onClick={() => {
            props.onClose();
            void copyConnectionUrl(conn);
          }}
        >
          Copy URL
        </button>
      </div>
    </div>
  );
}
