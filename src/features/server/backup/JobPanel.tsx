// Parts of the backup and restore dialogs: the file, the command that will run, and what
// the tool prints while it runs.

import { Copy, FolderOpen } from "lucide-react";
import { useEffect, useRef } from "react";
import { copyText } from "@/lib/clipboard";
import { toastSuccess } from "@/services/errors";

export function CommandPreview({ command }: { command: string }) {
  if (!command) return null;
  return (
    <div className="command-preview">
      <pre className="mono selectable">{command}</pre>
      <button
        className="btn xs"
        title="Copy command"
        onClick={() => void copyText(command).then(() => toastSuccess("Command copied."))}
      >
        <Copy size={12} /> Copy
      </button>
    </div>
  );
}

export function JobLog({ lines }: { lines: string[] }) {
  const ref = useRef<HTMLPreElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);
  if (lines.length === 0) return null;
  return (
    <pre ref={ref} className="job-log mono selectable" aria-label="Tool output">
      {lines.join("\n")}
    </pre>
  );
}

export function FileField(props: { path: string; onChoose: () => void; disabled: boolean }) {
  return (
    <div className="row tight">
      <input
        className="input grow mono"
        readOnly
        aria-label="File"
        placeholder="Choose a file…"
        value={props.path}
        onClick={() => !props.disabled && props.onChoose()}
      />
      <button className="btn" onClick={props.onChoose} disabled={props.disabled}>
        <FolderOpen size={13} /> Choose…
      </button>
    </div>
  );
}
