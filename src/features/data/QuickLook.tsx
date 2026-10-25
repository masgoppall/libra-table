// Quick Look (Space on a cell): the whole value — image, JSON tree, text or hex.

import { Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { ErrorCallout } from "@/components/ErrorCallout";
import { JsonTree, parseJson } from "@/components/JsonTree";
import { Modal } from "@/components/Modal";
import { Spinner } from "@/components/Spinner";
import { copyText } from "@/lib/clipboard";
import { formatBytes } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type AppError, type CellContent } from "@/types";

export interface QuickLookProps {
  title: string;
  /** Loads the whole value (the grid may only hold a preview). */
  load: () => Promise<CellContent>;
  onClose: () => void;
}

const HEX_BYTES = 4096;

function hexDump(base64: string): string {
  const bytes = Uint8Array.from(atob(base64).slice(0, HEX_BYTES), (c) => c.charCodeAt(0));
  const lines: string[] = [];
  for (let i = 0; i < bytes.length; i += 16) {
    const row = Array.from(bytes.slice(i, i + 16), (b) => b.toString(16).padStart(2, "0"));
    lines.push(`${i.toString(16).padStart(8, "0")}  ${row.join(" ")}`);
  }
  return lines.join("\n");
}

function TextView({ text }: { text: string }) {
  const [tree, setTree] = useState(true);
  const json = parseJson(text);
  if (json === null) return <pre className="quicklook-text mono selectable">{text}</pre>;
  return (
    <>
      <div className="segmented">
        <button className={tree ? "active" : ""} onClick={() => setTree(true)}>
          Tree
        </button>
        <button className={tree ? "" : "active"} onClick={() => setTree(false)}>
          Text
        </button>
      </div>
      {tree ? (
        <JsonTree value={json} />
      ) : (
        <pre className="quicklook-text mono selectable">{JSON.stringify(json, null, 2)}</pre>
      )}
    </>
  );
}

function BinaryView({ c }: { c: Extract<CellContent, { kind: "binary" }> }) {
  const note = `${formatBytes(c.size)}${c.truncated ? " · shown in part" : ""}`;
  return (
    <>
      <div className="faint small">
        {c.mime ?? "binary"} · {note}
      </div>
      {c.mime?.startsWith("image/") ? (
        <img className="quicklook-image" src={`data:${c.mime};base64,${c.base64}`} alt="" />
      ) : (
        <pre className="quicklook-text mono selectable">{hexDump(c.base64)}</pre>
      )}
    </>
  );
}

function copyable(c: CellContent): string {
  if (c.kind === "text") return c.text;
  if (c.kind === "binary") return c.base64;
  return "";
}

export function QuickLook(props: QuickLookProps) {
  const [content, setContent] = useState<CellContent | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  useEffect(() => {
    props.load().then(setContent, (e) => setError(e as AppError));
  }, []);
  const copy = async () => {
    if (!content) return;
    await copyText(copyable(content));
    toastSuccess(content.kind === "binary" ? "Copied as Base64." : "Value copied.");
  };
  return (
    <Modal
      title={props.title}
      size="xl"
      onClose={props.onClose}
      footer={
        <>
          <button className="btn" onClick={() => void copy()} disabled={!content}>
            <Copy size={13} /> Copy
          </button>
          <span className="spacer" />
          <button className="btn primary" onClick={props.onClose}>
            Done
          </button>
        </>
      }
    >
      <ErrorCallout error={error} />
      {!content && !error && <Spinner />}
      {content?.kind === "null" && <div className="faint">NULL</div>}
      {content?.kind === "text" && <TextView text={content.text} />}
      {content?.kind === "binary" && <BinaryView c={content} />}
    </Modal>
  );
}
