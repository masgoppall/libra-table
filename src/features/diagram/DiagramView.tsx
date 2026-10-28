// ER diagram tab (DiagramWindow): the schema's tables and their foreign keys.

import { save } from "@tauri-apps/plugin-dialog";
import {
  Download,
  Image as ImageIcon,
  LayoutGrid,
  Maximize,
  Network,
  RefreshCw,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { api } from "@/api";
import { PaddedError } from "@/components/ErrorCallout";
import { EmptyState, LoadingState } from "@/components/StateView";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { tableTab } from "../workspace/tabs";
import { DiagramCanvas } from "./DiagramCanvas";
import { diagramSvg, exportSize, svgToPng } from "./diagramExport";
import { bounds, rectOf } from "./layout";
import { type DiagramState, useDiagram } from "./useDiagram";
import { useViewport } from "./useViewport";

async function exportAs(kind: "svg" | "png", svg: SVGSVGElement, d: DiagramState) {
  const tables = d.data?.tables ?? [];
  const content = bounds(tables.map((t) => rectOf(t, d.layout)));
  const background = getComputedStyle(document.body).backgroundColor || "#ffffff";
  const text = diagramSvg(svg, content, background);
  const path = await save({
    defaultPath: `${d.schema}-diagram.${kind}`,
    filters: [{ name: kind.toUpperCase(), extensions: [kind] }],
  });
  if (!path) return;
  const { w, h } = exportSize(content);
  const file = kind === "svg" ? { svg: text } : { pngBase64: await svgToPng(text, w, h) };
  await api.saveDiagramFile(path, file);
  toastSuccess("Diagram exported.", path);
}

export function DiagramView({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const d = useDiagram(ws, tab);
  const tables = d.data?.tables ?? [];
  const vp = useViewport(tables.length > 0);
  const svgRef = useRef<SVGSVGElement>(null);
  const fit = () => vp.fit(bounds(tables.map((t) => rectOf(t, d.layout))));
  // Fit once the tables first arrive.
  const loaded = d.data !== null;
  useEffect(() => {
    if (loaded) fit();
  }, [loaded]);
  const open = (name: string) => {
    const where = { schema: d.schema, database: tab.database };
    store.get().addTab(ws.connectionId, tableTab("data", name, where), { reuse: true });
  };
  const doExport = (kind: "svg" | "png") => {
    if (svgRef.current) exportAs(kind, svgRef.current, d).catch(showError);
  };
  const relations = tables.reduce((n, t) => n + t.foreignKeys.length, 0);

  return (
    <div className="diagram-view">
      <div className="toolbar">
        <div className="toolbar-title">
          <Network size={15} />
          <strong>ER diagram</strong>
          <span className="muted">{d.schema}</span>
        </div>
        <span className="spacer" />
        <div className="tool-group">
          <button className="btn sm icon" title="Zoom out" onClick={() => vp.zoomBy(1 / 1.2)}>
            <ZoomOut size={14} />
          </button>
          <span className="diagram-zoom">{Math.round(vp.view.scale * 100)}%</span>
          <button className="btn sm icon" title="Zoom in" onClick={() => vp.zoomBy(1.2)}>
            <ZoomIn size={14} />
          </button>
          <button className="btn sm" title="Fit the diagram" onClick={fit}>
            <Maximize size={13} /> Fit
          </button>
        </div>
        <span className="tool-sep" />
        <button className="btn sm" title="Arrange automatically" onClick={d.reset}>
          <LayoutGrid size={13} /> Auto layout
        </button>
        <button className="btn sm" onClick={() => doExport("png")}>
          <ImageIcon size={13} /> PNG
        </button>
        <button className="btn sm" onClick={() => doExport("svg")}>
          <Download size={13} /> SVG
        </button>
        <button className="btn sm icon" title="Reload" onClick={d.reload}>
          <RefreshCw size={13} />
        </button>
      </div>
      <PaddedError error={d.error} />
      {!d.data && !d.error && <LoadingState label="Loading the schema…" />}
      {d.data && tables.length === 0 && (
        <EmptyState
          art="no-tables"
          icon={<Network size={20} />}
          title="No tables yet"
          hint="Tables of this schema and their relationships appear here."
        />
      )}
      {d.data && tables.length > 0 && (
        <DiagramCanvas
          tables={tables}
          layout={d.layout}
          viewport={vp}
          svgRef={svgRef}
          onMove={d.move}
          onMoved={d.persist}
          onOpen={open}
        />
      )}
      <div className="statusbar">
        <span>
          {plural(tables.length, "table")} · {plural(relations, "relationship")}
          {d.data?.truncated ? " (first 1,000 tables)" : ""}
        </span>
        <span className="spacer" />
        <span className="faint">
          Drag tables to arrange · drag the background to pan · double-click a table to open it
        </span>
      </div>
    </div>
  );
}
