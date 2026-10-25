// Settings of one widget: title, type, query and the options of its type.

import { useState } from "react";
import { Modal } from "@/components/Modal";
import { ModalActions } from "@/components/ModalActions";
import { type MetricWidget, type ValueFormat, type WidgetKind, type WidgetOptions } from "@/types";
import { WIDGET_KINDS } from "./boardModel";

const FORMATS: [ValueFormat, string][] = [
  ["auto", "Automatic"],
  ["number", "Number"],
  ["percent", "Percent (0.25 → 25%)"],
  ["currency", "Currency"],
  ["bytes", "Bytes"],
];

const isChart = (k: WidgetKind) => k === "bar" || k === "line" || k === "pie";

function KindOptions(props: { w: MetricWidget; set: (o: Partial<WidgetOptions>) => void }) {
  const { w, set } = props;
  const o = w.options;
  if (w.kind === "input") {
    return (
      <>
        <label>Variable</label>
        <input
          className="input mono"
          aria-label="Variable name"
          value={o.variable}
          onChange={(e) => set({ variable: e.target.value.replace(/[^A-Za-z0-9_]/g, "") })}
        />
        <label>Default value</label>
        <input
          className="input"
          aria-label="Default value"
          value={o.defaultValue}
          onChange={(e) => set({ defaultValue: e.target.value })}
        />
        <label>Kind</label>
        <select
          className="select"
          value={o.inputType}
          onChange={(e) => set({ inputType: e.target.value as WidgetOptions["inputType"] })}
        >
          <option value="text">Text (sent as a quoted literal)</option>
          <option value="number">Number (must be a number)</option>
        </select>
      </>
    );
  }
  return (
    <>
      {isChart(w.kind) && (
        <>
          <label>Label column</label>
          <input
            className="input mono"
            placeholder="first column"
            value={o.labelColumn}
            onChange={(e) => set({ labelColumn: e.target.value.trim() })}
          />
        </>
      )}
      {(isChart(w.kind) || w.kind === "scoreboard") && (
        <>
          <label>{w.kind === "scoreboard" ? "Value column" : "Value columns"}</label>
          <input
            className="input mono"
            placeholder={
              w.kind === "scoreboard" ? "first column" : "numeric columns, comma-separated"
            }
            value={o.valueColumns.join(", ")}
            onChange={(e) =>
              set({
                valueColumns: e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean),
              })
            }
          />
          <label>Format</label>
          <select
            className="select"
            value={o.format}
            onChange={(e) => set({ format: e.target.value as ValueFormat })}
          >
            {FORMATS.map(([f, label]) => (
              <option key={f} value={f}>
                {label}
              </option>
            ))}
          </select>
        </>
      )}
      {w.kind === "scoreboard" && (
        <>
          <label>Caption</label>
          <input
            className="input"
            value={o.label}
            onChange={(e) => set({ label: e.target.value })}
          />
        </>
      )}
    </>
  );
}

export function WidgetEditor(props: {
  widget: MetricWidget;
  onSave: (w: MetricWidget) => void;
  onClose: () => void;
}) {
  const [w, setW] = useState(props.widget);
  const set = (patch: Partial<MetricWidget>) => setW((x) => ({ ...x, ...patch }));
  // An input still titled after its variable keeps following the variable's name.
  const setOptions = (o: Partial<WidgetOptions>) =>
    setW((x) => {
      const follow = o.variable !== undefined && x.title === x.options.variable;
      return {
        ...x,
        title: follow ? (o.variable ?? "") : x.title,
        options: { ...x.options, ...o },
      };
    });
  const invalid = w.kind === "input" && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(w.options.variable);
  return (
    <Modal
      title="Widget settings"
      size="lg"
      onClose={props.onClose}
      footer={
        <ModalActions
          onCancel={props.onClose}
          onOk={() => props.onSave(w)}
          okLabel="Save widget"
          disabled={invalid}
        />
      }
    >
      <div className="form-grid">
        <label>Title</label>
        <input
          className="input"
          aria-label="Widget title"
          value={w.title}
          onChange={(e) => set({ title: e.target.value })}
        />
        <label>Type</label>
        <select
          className="select"
          aria-label="Widget type"
          value={w.kind}
          onChange={(e) => set({ kind: e.target.value as WidgetKind })}
        >
          {WIDGET_KINDS.map(([k, label]) => (
            <option key={k} value={k}>
              {label}
            </option>
          ))}
        </select>
        {w.kind !== "input" && (
          <>
            <label>Query</label>
            <textarea
              className="input mono metric-sql"
              aria-label="Widget query"
              spellCheck={false}
              value={w.sql}
              onChange={(e) => set({ sql: e.target.value })}
            />
          </>
        )}
        <KindOptions w={w} set={setOptions} />
      </div>
      {w.kind !== "input" && (
        <div className="faint small">
          Read-only queries only. Use an input widget's value as <code>{"{{name}}"}</code>; text
          values become quoted literals, so write <code>{"WHERE region = {{region}}"}</code>.
        </div>
      )}
    </Modal>
  );
}
