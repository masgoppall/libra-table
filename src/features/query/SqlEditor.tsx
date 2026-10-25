// CodeMirror with the driver's dialect, schema completion, the app's run/save keys and
// the marks of the last run (flash, error gutter). ⌘/ comments lines (default keymap).

import { MySQL, PostgreSQL, SQLite, sql as sqlLang } from "@codemirror/lang-sql";
import { Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { type RefObject, useMemo } from "react";
import { useDarkMode } from "@/hooks/useDarkMode";
import { dispatch, shortcutFor } from "@/lib/actions";
import { useSettings } from "@/store";
import { redisEditor } from "../keyvalue/redisEditor";
import { runMarks } from "./editor/runMarks";
import { type CompletionSchema, type Dialect } from "@/types";

const DIALECTS = { postgres: PostgreSQL, mysql: MySQL, sqlite: SQLite };

/** Placeholder examples for languages other than SQL. */
const EXAMPLES: Record<string, string> = {
  redis: "GET key",
  mongo: "db.collection.find({})",
  partiql: 'SELECT * FROM "table"',
  elastic: "GET /index/_search",
};

// CodeMirror sees these keys first; hand them to the app's actions.
const appKeys = Prec.highest(
  keymap.of([
    { key: "Mod-Enter", run: () => dispatch("run-current", "key") },
    { key: "Shift-Mod-Enter", run: () => dispatch("run-all", "key") },
    { key: "Mod-s", run: () => dispatch("save", "key"), preventDefault: true },
    { key: "Mod-.", run: () => dispatch("cancel-query", "key") },
    { key: "Mod-Alt-e", run: () => dispatch("explain", "key"), preventDefault: true },
    { key: "Mod-Alt-f", run: () => dispatch("format-sql", "key"), preventDefault: true },
  ]),
);

function editorTheme(fontSize: number) {
  return EditorView.theme({
    "&": { fontSize: `${fontSize}px` },
    ".cm-content": { fontFamily: "var(--mono)" },
    ".cm-gutters": { fontFamily: "var(--mono)" },
  });
}

export interface SqlEditorProps {
  editorRef: RefObject<ReactCodeMirrorRef>;
  value: string;
  onChange: (v: string) => void;
  driver: Dialect;
  /** The driver's query language (caps.queryLanguage); only "sql" gets SQL support. */
  language: string;
  schema: string;
  completion: CompletionSchema | null;
}

export function SqlEditor(p: SqlEditorProps) {
  const runKey = shortcutFor("run-current");
  const isSql = p.language === "sql";
  const example = isSql ? "SELECT * FROM …" : (EXAMPLES[p.language] ?? "");
  const placeholder = `${example}    ${runKey} runs the statement under the cursor`;
  const settings = useSettings();
  const dark = useDarkMode(settings?.theme);
  const fontSize = settings?.editorFontSize ?? 13;
  const extensions = useMemo(
    () => [
      ...(isSql
        ? [
            sqlLang({
              dialect: DIALECTS[p.driver],
              schema: p.completion?.tables ?? {},
              defaultSchema: p.schema,
              upperCaseKeywords: true,
            }),
          ]
        : []),
      ...(p.language === "redis" ? [redisEditor()] : []),
      appKeys,
      runMarks,
      editorTheme(fontSize),
      EditorView.lineWrapping,
    ],
    [p.completion, p.driver, p.schema, fontSize, isSql, p.language],
  );
  return (
    <CodeMirror
      ref={p.editorRef}
      value={p.value}
      height="100%"
      theme={dark ? "dark" : "light"}
      extensions={extensions}
      onChange={p.onChange}
      autoFocus
      basicSetup={{ highlightActiveLine: true, foldGutter: true, autocompletion: true }}
      placeholder={placeholder}
    />
  );
}
