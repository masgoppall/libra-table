// "Ask AI" above the query editor: a question in plain language becomes SQL, inserted
// at the cursor (or over the selection) in one undoable step.

import { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { Sparkles, X } from "lucide-react";
import { type RefObject, useEffect, useRef, useState } from "react";
import { api } from "@/api";
import { Spinner } from "@/components/Spinner";
import { plural } from "@/lib/format";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type CompletionSchema, type Workspace } from "@/types";
import { schemaSummary } from "./assistant";

export function AskBar(props: {
  ws: Workspace;
  completion: CompletionSchema | null;
  editorRef: RefObject<ReactCodeMirrorRef>;
  focusSignal: number;
  onClose: () => void;
}) {
  const [question, setQuestion] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), [props.focusSignal]);

  const ask = async () => {
    const view = props.editorRef.current?.view;
    if (!question.trim() || !view || busy) return;
    const sel = view.state.selection.main;
    const currentSql = sel.empty
      ? view.state.doc.toString()
      : view.state.sliceDoc(sel.from, sel.to);
    const schema = schemaSummary(props.completion);
    const info = props.ws.info;
    setBusy(true);
    setNote("");
    try {
      const a = await api.aiAsk({
        question,
        dialect: `${info.driver} ${info.serverVersion}`,
        schema: schema.text,
        currentSql: currentSql.slice(0, 20_000),
        model: store.get().settings?.aiModel,
      });
      if (a.sql) {
        const text = sel.empty && view.state.doc.length ? `\n\n${a.sql}\n` : a.sql;
        const at = sel.empty ? view.state.doc.length : sel.from;
        view.dispatch({
          changes: { from: at, to: sel.empty ? at : sel.to, insert: text },
          selection: { anchor: at + text.length },
          userEvent: "input",
        });
        view.focus();
      }
      const partial =
        schema.sent < schema.total
          ? ` Only ${schema.sent} of ${plural(schema.total, "table")} fit in the request.`
          : "";
      setNote(`${a.note}${partial}`.trim());
    } catch (e) {
      showError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="ask-bar">
      <div className="row">
        <Sparkles size={14} className="ask-icon" />
        <input
          ref={input}
          className="input grow"
          aria-label="Ask the SQL assistant"
          placeholder="Describe the query, e.g. “customers who ordered last month, newest first”"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void ask();
            } else if (e.key === "Escape") props.onClose();
          }}
        />
        <button
          className="btn sm primary"
          disabled={busy || !question.trim()}
          onClick={() => void ask()}
        >
          {busy ? <Spinner size={12} /> : null} Write SQL
        </button>
        <button
          className="btn sm icon ghost"
          aria-label="Close the assistant"
          onClick={props.onClose}
        >
          <X size={13} />
        </button>
      </div>
      {note && <div className="ask-note small muted">{note}</div>}
    </div>
  );
}
