// Metrics tab (MetricsBoardWindow): boards of widgets, each running a read-only query.

import { Gauge, LayoutGrid } from "lucide-react";
import { useEffect, useState } from "react";
import { EmptyState, LoadingState } from "@/components/StateView";
import { tabDb } from "@/store";
import { type MetricBoard, type MetricWidget, type Workspace, type WorkspaceTab } from "@/types";
import * as model from "./boardModel";
import { BoardToolbar } from "./BoardToolbar";
import { useBoards } from "./useBoards";
import { WidgetCard } from "./WidgetCard";
import { WidgetEditor } from "./WidgetEditor";

/** Bumps a counter every `seconds` while the tab is on screen. */
function useRefreshTick(seconds: number, visible: boolean) {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!seconds || !visible) return;
    const t = setInterval(() => setTick((n) => n + 1), seconds * 1000);
    return () => clearInterval(t);
  }, [seconds, visible]);
  return { tick, bump: () => setTick((n) => n + 1) };
}

function Empty({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="metric-empty-board">
      <Gauge size={28} className="faint" />
      <div>Boards show numbers, tables and charts from your queries.</div>
      <button className="btn primary" onClick={onCreate}>
        New board
      </button>
    </div>
  );
}

function Grid(props: {
  ws: Workspace;
  tab: WorkspaceTab;
  board: MetricBoard;
  editing: boolean;
  tick: number;
  save: (b: MetricBoard) => void;
  configure: (w: MetricWidget) => void;
}) {
  const { board, save } = props;
  const [values, setValues] = useState<Record<string, string>>({});
  const variables = model.boardVariables(board, values);
  const actions = (w: MetricWidget) => ({
    move: (dir: -1 | 1) => save(model.moveWidget(board, w.id, dir)),
    resize: (dw: number, dh: number) => save(model.resizeWidget(board, w.id, dw, dh)),
    configure: () => props.configure(w),
    remove: () => save(model.removeWidget(board, w.id)),
  });
  if (board.widgets.length === 0) {
    const hint = props.editing ? "Add a widget above." : "Click Edit, then Add widget.";
    return (
      <EmptyState
        art="board"
        icon={<LayoutGrid size={20} />}
        title="This board is empty"
        hint={hint}
      />
    );
  }
  return (
    <div className="metric-grid">
      {board.widgets.map((w) => (
        <WidgetCard
          key={w.id}
          connectionId={props.ws.connectionId}
          db={tabDb(props.ws, props.tab)}
          w={w}
          variables={variables}
          tick={props.tick}
          editing={props.editing}
          actions={actions(w)}
          value={values[w.options.variable] ?? w.options.defaultValue}
          onValue={(v) => setValues((x) => ({ ...x, [w.options.variable]: v }))}
        />
      ))}
    </div>
  );
}

export function MetricsView(props: { ws: Workspace; tab: WorkspaceTab; visible: boolean }) {
  const boards = useBoards(props.ws.connectionId);
  const board = boards.active;
  const [editing, setEditing] = useState(false);
  const [configuring, setConfiguring] = useState<MetricWidget | null>(null);
  const refresh = useRefreshTick(board?.refreshSeconds ?? 0, props.visible);

  const add = (kind: MetricWidget["kind"]) => {
    if (!board) return;
    const taken = board.widgets.map((w) => w.options.variable);
    const w = model.newWidget(kind, taken);
    boards.save({ ...board, widgets: [...board.widgets, w] });
    setConfiguring(w);
  };

  return (
    <div className="metrics-view">
      <BoardToolbar
        boards={boards}
        board={board}
        editing={editing}
        setEditing={setEditing}
        onRefresh={refresh.bump}
        onAdd={add}
      />
      <div className="metrics-body">
        {boards.boards === null && <LoadingState label="Loading boards…" />}
        {boards.boards?.length === 0 && <Empty onCreate={() => void boards.create()} />}
        {board && (
          <Grid
            ws={props.ws}
            tab={props.tab}
            board={board}
            editing={editing}
            tick={refresh.tick}
            save={boards.save}
            configure={setConfiguring}
          />
        )}
      </div>
      {configuring && board && (
        <WidgetEditor
          widget={configuring}
          onClose={() => setConfiguring(null)}
          onSave={(w) => {
            boards.save(model.updateWidget(board, w));
            setConfiguring(null);
          }}
        />
      )}
    </div>
  );
}
