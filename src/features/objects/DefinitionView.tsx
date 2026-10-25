// A schema object's definition tab: its DDL (read-only), Copy, Edit in query tab, and
// the object's own actions (refresh a materialized view, reset a sequence, drop).

import { Copy, FileCode2, RefreshCw, RotateCcw, Table2, Trash2 } from "lucide-react";
import { PaddedError } from "@/components/ErrorCallout";
import { BusyIcon } from "@/components/Spinner";
import { KIND_LABEL, objectTitle } from "@/domain/objects";
import { LAYER, useActions, withShortcut } from "@/lib/actions";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";
import { useIsTabVisible } from "@/store";
import { type SchemaObjectRef, type Workspace, type WorkspaceTab } from "@/types";
import * as flows from "./objectFlows";
import { objectIcon } from "./objectKinds";
import { SequencePanel } from "./SequencePanel";
import { type DefinitionState, useDefinition } from "./useDefinition";

interface Props {
  ws: Workspace;
  tab: WorkspaceTab;
  object: SchemaObjectRef;
}

function KindActions({ ws, object, d }: Props & { d: DefinitionState }) {
  const where = { schema: d.schema };
  if (object.kind === "materialized_view") {
    return (
      <>
        <button className="btn sm" onClick={() => flows.openObject(ws, object, d.schema)}>
          <Table2 size={13} /> Open data
        </button>
        <button
          className="btn sm"
          onClick={() => void flows.refreshMaterializedView(ws, object.name, where)}
        >
          <RefreshCw size={13} /> Refresh…
        </button>
      </>
    );
  }
  if (object.kind !== "sequence") return null;
  const reset = async () => {
    const initial = d.sequence?.startValue;
    if (await flows.resetSequence(ws, object.name, { ...where, initial })) void d.reload();
  };
  return (
    <button className="btn sm" onClick={() => void reset()}>
      <RotateCcw size={13} /> Reset…
    </button>
  );
}

function Toolbar(p: Props & { d: DefinitionState }) {
  const { ws, object, d } = p;
  const Icon = objectIcon(object.kind);
  const copy = async () => {
    if (!d.definition) return;
    await copyText(d.definition.ddl);
    toastSuccess("Definition copied.");
  };
  return (
    <div className="toolbar">
      <span className="toolbar-title">
        <Icon size={14} />
        <strong className="ellipsis selectable">{objectTitle(object)}</strong>
        <span className="readonly-note">{KIND_LABEL[object.kind]}</span>
      </span>
      <span className="spacer" />
      <KindActions {...p} />
      <button className="btn sm" onClick={() => void copy()} disabled={!d.definition}>
        <Copy size={13} /> Copy
      </button>
      <button
        className="btn sm"
        onClick={() => void flows.editInQueryTab(ws, object, d.schema)}
        title="Open a query tab with a script that changes this object"
      >
        <FileCode2 size={13} /> Edit in query tab
      </button>
      <button
        className="btn sm icon"
        onClick={() => void d.reload()}
        title={withShortcut("Reload", "refresh")}
      >
        <BusyIcon busy={d.loading} icon={<RefreshCw size={13} />} />
      </button>
      <button
        className="btn sm icon danger-outline"
        onClick={() => void flows.dropObject(ws, object, d.schema)}
        title={`Drop ${KIND_LABEL[object.kind]}…`}
      >
        <Trash2 size={13} />
      </button>
    </div>
  );
}

export function DefinitionView(p: Props) {
  const d = useDefinition(p.ws, p.tab, p.object);
  const visible = useIsTabVisible(p.ws.connectionId, p.tab.id);
  useActions(LAYER.tab, visible, { refresh: () => void d.reload() });
  return (
    <div className="tab-body">
      <Toolbar {...p} d={d} />
      <PaddedError error={d.error} />
      <div className="designer definition">
        {d.sequence && <SequencePanel info={d.sequence} />}
        {d.definition && (
          <div className="sql-box">
            <div className="sql-box-head">Definition</div>
            <pre className="mono selectable definition-ddl">{d.definition.ddl}</pre>
          </div>
        )}
      </div>
    </div>
  );
}
