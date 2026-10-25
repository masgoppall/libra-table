// What the query tab's buttons and keys do: run the statement at the cursor (it flashes)
// or everything, EXPLAIN it, and beautify. Runs ask for `:name` variables first, and
// statements that fail get a gutter marker.

import { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { type EditorView } from "@codemirror/view";
import { type RefObject } from "react";
import { api } from "@/api";
import { canAnalyze, explainSql } from "@/lib/explainPlan";
import {
  rememberVariables,
  substituteVariables,
  variableNames,
  variablesFor,
} from "@/lib/variables";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type QueryVariable, type Statement, type Workspace, type WorkspaceTab } from "@/types";
import { flashRange, markErrors } from "./editor/runMarks";
import { formatSql } from "./format";
import { type ExplainSpec } from "./results";
import { statementAt } from "./statements";
import { type QueryRunner } from "./useQueryRunner";

/** Statements that only read: ANALYZE of these needs no warning. */
const READ_ONLY = /^\s*(select|values|table|show)\b/i;

interface Target {
  text: string;
  /** Document offset of `text`. */
  from: number;
}

export interface QueryActionDeps {
  ws: Workspace;
  tab: WorkspaceTab;
  editorRef: RefObject<ReactCodeMirrorRef>;
  runner: QueryRunner;
  askVariables: (vars: QueryVariable[]) => Promise<QueryVariable[] | null>;
}

export function useQueryActions(d: QueryActionDeps) {
  const driver = d.ws.info.driver;
  const split = (text: string) => api.splitStatements(driver, text);

  /** The selection if any, else the statement under the cursor. */
  const target = async (view: EditorView): Promise<Target | null> => {
    const sel = view.state.selection.main;
    if (!sel.empty) return { text: view.state.sliceDoc(sel.from, sel.to), from: sel.from };
    const st = statementAt(await split(view.state.doc.toString()), sel.head);
    return st ? { text: st.text, from: st.from } : null;
  };

  /** `text` with its variables filled in; null when cancelled or invalid. */
  const fillVariables = async (text: string): Promise<string | null> => {
    const names = variableNames(text, driver);
    if (!names.length) return text;
    const remembered = store.workspace(d.ws.connectionId)?.tabs.find((t) => t.id === d.tab.id);
    const values = await d.askVariables(variablesFor(names, remembered?.variables));
    if (!values) return null;
    const variables = rememberVariables(remembered?.variables, values);
    store.get().updateTab(d.ws.connectionId, d.tab.id, { variables });
    try {
      return substituteVariables(text, driver, values);
    } catch (e) {
      store.get().toast("error", (e as Error).message);
      return null;
    }
  };

  /** Runs `sql` (statements `parts` of the text at `from`) and marks the failures. */
  const runMarked = async (
    view: EditorView,
    parts: Statement[],
    from: number,
    sql: string,
    explain?: ExplainSpec,
  ) => {
    const res = await d.runner.run(sql, explain);
    if (!res) return;
    const marks = res.results.flatMap((r, i) => {
      const part = parts[i];
      return r.error && part ? [{ from: from + part.from, message: r.error.message }] : [];
    });
    markErrors(view, marks);
  };

  const run = async (whole: boolean) => {
    const view = d.editorRef.current?.view;
    if (!view || d.runner.running) return;
    try {
      const t = whole ? { text: view.state.doc.toString(), from: 0 } : await target(view);
      if (!t || !t.text.trim()) return;
      if (!whole) flashRange(view, t.from, t.from + t.text.length);
      const parts = await split(t.text);
      const sql = await fillVariables(t.text);
      if (sql !== null) await runMarked(view, parts, t.from, sql);
    } catch (e) {
      showError(e);
    }
  };

  const confirmAnalyze = async (statement: string) => {
    if (READ_ONLY.test(statement)) return true;
    const r = await store.get().dialog({
      kind: "confirm",
      title: "Explain analyze",
      message:
        "ANALYZE runs the statement to measure it: an INSERT, UPDATE or DELETE really " +
        "changes data. On a production connection safe mode asks again.",
      details: [statement],
      okLabel: "Run and explain",
      danger: true,
    });
    return r.ok;
  };

  const explain = async (analyze: boolean) => {
    const view = d.editorRef.current?.view;
    if (!view || d.runner.running) return;
    const withAnalyze = analyze && canAnalyze(driver);
    try {
      const t = await target(view);
      const first = t && (await split(t.text))[0];
      if (!t || !first) return;
      flashRange(view, t.from + first.from, t.from + first.to);
      if (withAnalyze && !(await confirmAnalyze(first.text))) return;
      const filled = await fillVariables(first.text);
      if (filled === null) return;
      const sql = explainSql(driver, filled, withAnalyze);
      await runMarked(view, [first], t.from, sql, { driver, analyze: withAnalyze });
    } catch (e) {
      showError(e);
    }
  };

  const format = () => {
    const view = d.editorRef.current?.view;
    if (!view) return;
    const sel = view.state.selection.main;
    const [from, to] = sel.empty ? [0, view.state.doc.length] : [sel.from, sel.to];
    try {
      const pretty = formatSql(view.state.sliceDoc(from, to), driver);
      view.dispatch({ changes: { from, to, insert: pretty } });
    } catch (e) {
      store.get().toast("error", "This SQL couldn't be formatted.", (e as Error).message);
    }
  };

  return { runCurrent: () => run(false), runAll: () => run(true), explain, format };
}
