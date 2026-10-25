// A collapsible tree of a JSON value (TablePlus JSON viewer).

import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

/** Objects and arrays deeper than this start folded. */
const OPEN_DEPTH = 2;

function Scalar({ value }: { value: Json }) {
  if (value === null) return <span className="json-null">null</span>;
  if (typeof value === "string") return <span className="json-string">"{value}"</span>;
  return <span className={`json-${typeof value}`}>{String(value)}</span>;
}

function summary(value: Json[] | { [key: string]: Json }): string {
  return Array.isArray(value) ? `[${value.length}]` : `{${Object.keys(value).length}}`;
}

function Node(props: { name?: string; value: Json; depth: number }) {
  const { value, depth } = props;
  const [open, setOpen] = useState(depth < OPEN_DEPTH);
  const label = props.name !== undefined && <span className="json-key">{props.name}: </span>;
  if (value === null || typeof value !== "object") {
    return (
      <div className="json-row">
        {label}
        <Scalar value={value} />
      </div>
    );
  }
  const entries = Array.isArray(value)
    ? value.map((v, i) => [String(i), v] as const)
    : Object.entries(value);
  return (
    <div>
      <div className="json-row json-branch" onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        {label}
        <span className="faint">{summary(value)}</span>
      </div>
      {open && (
        <div className="json-children">
          {entries.map(([k, v]) => (
            <Node key={k} name={k} value={v} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

/** Null when `text` isn't a JSON object or array. */
export function parseJson(text: string): Json | null {
  const t = text.trim();
  if (!(t.startsWith("{") || t.startsWith("["))) return null;
  try {
    return JSON.parse(t) as Json;
  } catch {
    return null;
  }
}

export function JsonTree({ value }: { value: Json }) {
  return (
    <div className="json-tree mono selectable">
      <Node value={value} depth={0} />
    </div>
  );
}
