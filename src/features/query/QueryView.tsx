// SQL editor tab (QRY-1..QRY-4, QRY-7). Each query tab runs in its own database.

import { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { useEffect, useRef, useState } from "react";
import { isProduction } from "@/domain/connection";
import { defaultSchema } from "@/domain/schema";
import { useDebouncedEffect } from "@/hooks/useDebouncedEffect";
import { LAYER, useActions } from "@/lib/actions";
import { canAnalyze } from "@/lib/explainPlan";
import { store, useConnection, useIsTabVisible, useStore } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { ExportDialog } from "../export/ExportDialog";
import { QueryToolbar } from "./QueryToolbar";
import { ResultsPane } from "./ResultsPane";
import { saveQuery } from "./saveQuery";
import { convertIndentation, transformSelection, trimTrailingWhitespace } from "./editor/editing";
import { AskBar } from "./AskBar";
import { SqlEditor } from "./SqlEditor";
import { useEditorSplit } from "./useEditorSplit";
import { useQueryActions } from "./useQueryActions";
import { useQueryContext } from "./useQueryContext";
import { useQueryRunner } from "./useQueryRunner";
import { useVariablePrompt } from "./useVariablePrompt";

/** Keep the editor text in the tab (restored next time) without saving on every key. */
const SAVE_TEXT_MS = 500;

export function QueryView({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const id = ws.connectionId;
  const conn = useConnection(id);
  const visible = useIsTabVisible(id, tab.id);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const [text, setText] = useState(tab.sql ?? "");
  const [exportSql, setExportSql] = useState<string | null>(null);
  const ctx = useQueryContext(ws, tab);
  const runner = useQueryRunner(id, ctx.db);
  const variables = useVariablePrompt();
  const actions = useQueryActions({ ws, tab, editorRef, runner, askVariables: variables.prompt });
  const split = useEditorSplit();
  const { updateTab } = store.get();

  useDebouncedEffect(() => updateTab(id, tab.id, { sql: text }), [text], SAVE_TEXT_MS);

  const save = () => saveQuery(id, tab, text);
  const result = runner.result;
  const canExport = !!result?.columns.length && !result.error && !runner.item?.explain;

  useActions(LAYER.tab, visible, {
    "run-current": () => void actions.runCurrent(),
    "run-all": () => void actions.runAll(),
    "cancel-query": runner.cancel,
    explain: () => (ws.caps.explain ? void actions.explain(false) : false),
    "format-sql": () => (ws.caps.sqlDialect ? actions.format() : false),
    "trim-whitespace": () => {
      const view = editorRef.current?.view;
      return view ? void trimTrailingWhitespace(view) : false;
    },
    save: () => void save(),
    export: () => (canExport && result ? setExportSql(result.sql) : false),
    // Results don't refresh on ⌘R; the sidebar does (next layer).
    refresh: () => false,
    "ask-ai": () => (aiOn ? openAsk() : false),
  });

  // "Run again" from history: run once when the tab first shows.
  useEffect(() => {
    if (!tab.autoRun || !text.trim()) return;
    updateTab(id, tab.id, { autoRun: undefined });
    void runner.run(text);
  }, []);

  const aiOn = useStore((st) => !!st.settings?.aiEnabled) && ws.caps.sqlDialect !== null;
  const [asking, setAsking] = useState(0);
  const openAsk = () => setAsking((n) => n + 1);

  const pickDatabase = (d: string) =>
    updateTab(id, tab.id, { database: d, schema: defaultSchema(ws.info.dialect, d) });

  return (
    <div className="tab-body">
      <QueryToolbar
        running={!!runner.running}
        production={isProduction(conn)}
        saved={!!tab.savedName}
        databases={ctx.databases}
        database={ctx.database}
        onDatabase={pickDatabase}
        onRunCurrent={() => void actions.runCurrent()}
        onRunAll={() => void actions.runAll()}
        onCancel={runner.cancel}
        onSave={() => void save()}
        onAsk={aiOn ? openAsk : undefined}
        tools={{
          running: !!runner.running,
          canExplain: ws.caps.explain,
          canFormat: ws.caps.sqlDialect !== null,
          canAnalyze: canAnalyze(ws.info.dialect),
          onExplain: (analyze) => void actions.explain(analyze),
          onFormat: actions.format,
          onTool: (t) => {
            const view = editorRef.current?.view;
            if (!view) return;
            if (t.kind === "trim") trimTrailingWhitespace(view);
            else if (t.kind === "indent") convertIndentation(view, t.to);
            else transformSelection(view, t.transform);
            view.focus();
          },
        }}
      />
      {aiOn && asking > 0 && (
        <AskBar
          ws={ws}
          completion={ctx.completion}
          editorRef={editorRef}
          focusSignal={asking}
          onClose={() => setAsking(0)}
        />
      )}
      <div className="editor-split">
        <div className="editor-pane" style={{ height: `${split.height}%` }}>
          <SqlEditor
            editorRef={editorRef}
            value={text}
            onChange={setText}
            driver={ws.info.dialect}
            language={ws.caps.queryLanguage}
            schema={ctx.schema}
            completion={ctx.completion}
          />
        </div>
        <div className="splitter" onMouseDown={split.onSplitterDown} />
        <ResultsPane
          runner={runner}
          driver={ws.info.dialect}
          rowLimit={conn?.options.queryRowLimit ?? 0}
          onExport={setExportSql}
        />
      </div>
      {exportSql !== null && (
        <ExportDialog
          ws={ws}
          db={ctx.db}
          defaultName="query"
          querySql={exportSql}
          onClose={() => setExportSql(null)}
        />
      )}
      {variables.node}
    </div>
  );
}
