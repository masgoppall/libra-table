// One widget on the board: its title, its content, and in edit mode the controls to
// move, resize, configure and remove it.

import { ArrowLeft, ArrowRight, MoveDiagonal2, Minimize2, Settings2, Trash2 } from "lucide-react";
import { Spinner } from "@/components/Spinner";
import { type Db, type MetricVariable, type MetricWidget } from "@/types";
import { chartData } from "./chartScale";
import { useWidgetData } from "./useWidgetData";
import { InputWidget, Scoreboard, TableWidget } from "./widgets/Simple";
import { PieChart } from "./widgets/PieChart";
import { XYChart } from "./widgets/XYChart";

export interface WidgetActions {
  move: (dir: -1 | 1) => void;
  resize: (dw: number, dh: number) => void;
  configure: () => void;
  remove: () => void;
}

function Body(props: { w: MetricWidget; data: ReturnType<typeof useWidgetData> }) {
  const { w } = props;
  const { result, error } = props.data;
  if (error) return <div className="metric-error selectable">{error.message}</div>;
  if (!result)
    return <div className="metric-empty faint">{w.sql.trim() ? "" : "No query yet"}</div>;
  if (result.columns.length === 0) {
    return <div className="metric-empty faint">The query returned no columns.</div>;
  }
  switch (w.kind) {
    case "scoreboard":
      return <Scoreboard w={w} result={result} />;
    case "table":
      return <TableWidget result={result} />;
    case "bar":
    case "line":
      return (
        <XYChart data={chartData(result, w.options)} mode={w.kind} format={w.options.format} />
      );
    case "pie":
      return <PieChart data={chartData(result, w.options)} format={w.options.format} />;
    case "input":
      return null;
  }
}

function EditControls({ a }: { a: WidgetActions }) {
  const b = (title: string, onClick: () => void, icon: React.ReactNode) => (
    <button className="icon-btn" title={title} aria-label={title} onClick={onClick}>
      {icon}
    </button>
  );
  return (
    <span className="metric-controls">
      {b("Move earlier", () => a.move(-1), <ArrowLeft size={13} />)}
      {b("Move later", () => a.move(1), <ArrowRight size={13} />)}
      {b("Larger", () => a.resize(1, 1), <MoveDiagonal2 size={13} />)}
      {b("Smaller", () => a.resize(-1, -1), <Minimize2 size={13} />)}
      {b("Widget settings", a.configure, <Settings2 size={13} />)}
      {b("Remove widget", a.remove, <Trash2 size={13} />)}
    </span>
  );
}

export function WidgetCard(props: {
  connectionId: string;
  db: Db;
  w: MetricWidget;
  variables: MetricVariable[];
  tick: number;
  editing: boolean;
  actions: WidgetActions;
  value: string;
  onValue: (v: string) => void;
}) {
  const { w } = props;
  const data = useWidgetData(props.connectionId, props.db, w, props.variables, props.tick);
  const style = { gridColumn: `span ${w.width}`, gridRow: `span ${w.height}` };
  return (
    <section className={`metric-card ${w.kind}`} style={style} aria-label={w.title}>
      <header className="metric-head">
        <span className="metric-title ellipsis">{w.title}</span>
        {data.loading && <Spinner size={11} />}
        <span className="spacer" />
        {props.editing && <EditControls a={props.actions} />}
      </header>
      <div className="metric-body">
        {w.kind === "input" ? (
          <InputWidget w={w} value={props.value} onChange={props.onValue} />
        ) : (
          <Body w={w} data={data} />
        )}
      </div>
    </section>
  );
}
