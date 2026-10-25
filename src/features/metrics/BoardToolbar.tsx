// The metrics tab's toolbar: board picker, refresh, edit mode and adding widgets.

import { Check, Gauge, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useContextMenu } from "@/components/ContextMenu";
import { type MetricBoard, type WidgetKind } from "@/types";
import { REFRESH_CHOICES, WIDGET_KINDS } from "./boardModel";
import { type Boards } from "./useBoards";

export function BoardToolbar(props: {
  boards: Boards;
  board: MetricBoard | null;
  editing: boolean;
  setEditing: (e: boolean) => void;
  onRefresh: () => void;
  onAdd: (kind: WidgetKind) => void;
}) {
  const { boards: bs, board } = props;
  const menu = useContextMenu();
  const addMenu = WIDGET_KINDS.map(([k, label]) => ({ label, onClick: () => props.onAdd(k) }));
  return (
    <div className="toolbar">
      <div className="toolbar-title">
        <Gauge size={15} />
        <strong>Metrics</strong>
      </div>
      <select
        className="select sm"
        aria-label="Board"
        value={board?.id ?? ""}
        onChange={(e) => bs.setActiveId(e.target.value)}
        disabled={!bs.boards?.length}
      >
        {(bs.boards ?? []).map((b) => (
          <option key={b.id} value={b.id}>
            {b.name}
          </option>
        ))}
      </select>
      <button className="btn sm" onClick={() => void bs.create()}>
        <Plus size={13} /> New board
      </button>
      <span className="spacer" />
      {board && (
        <>
          <select
            className="select sm"
            aria-label="Auto refresh"
            title="Auto refresh"
            value={board.refreshSeconds}
            onChange={(e) => bs.save({ ...board, refreshSeconds: Number(e.target.value) })}
          >
            {REFRESH_CHOICES.map(([s, label]) => (
              <option key={s} value={s}>
                {s ? `Every ${label}` : "Auto refresh off"}
              </option>
            ))}
          </select>
          <button className="btn sm icon" title="Refresh now" onClick={props.onRefresh}>
            <RefreshCw size={13} />
          </button>
          <span className="tool-sep" />
          {props.editing && (
            <>
              <button className="btn sm" onClick={(e) => menu.openAt(e.currentTarget, addMenu)}>
                <Plus size={13} /> Add widget
              </button>
              <button
                className="btn sm icon"
                title="Rename board"
                onClick={() => void bs.rename(board)}
              >
                <Pencil size={13} />
              </button>
              <button
                className="btn sm icon danger-outline"
                title="Delete board"
                onClick={() => void bs.remove(board)}
              >
                <Trash2 size={13} />
              </button>
            </>
          )}
          <button
            className={`btn sm${props.editing ? " active" : ""}`}
            onClick={() => props.setEditing(!props.editing)}
          >
            {props.editing ? <Check size={13} /> : <Pencil size={13} />}
            {props.editing ? "Done" : "Edit"}
          </button>
        </>
      )}
      {menu.node}
    </div>
  );
}
