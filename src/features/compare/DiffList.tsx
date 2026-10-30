// The differences of a schema comparison, one checkbox per table.

import { type SchemaDiff } from "@/domain/schemaCompare";

function Row(props: {
  name: string;
  detail: string;
  kind: string;
  picked: Set<string>;
  onPicked: (s: Set<string>) => void;
}) {
  const on = props.picked.has(props.name);
  const toggle = () => {
    const next = new Set(props.picked);
    if (on) next.delete(props.name);
    else next.add(props.name);
    props.onPicked(next);
  };
  return (
    <label className={`compare-row ${props.kind}`}>
      <input type="checkbox" checked={on} onChange={toggle} />
      <span className="compare-kind">{props.kind}</span>
      <span className="mono">{props.name}</span>
      <span className="faint small">{props.detail}</span>
    </label>
  );
}

export function DiffList(p: {
  diff: SchemaDiff;
  picked: Set<string>;
  onPicked: (s: Set<string>) => void;
}) {
  const { diff } = p;
  const empty = !diff.create.length && !diff.drop.length && !diff.changed.length;
  if (empty) return <div className="callout ok">The schemas have the same tables and columns.</div>;
  return (
    <div className="compare-list" role="list" aria-label="Differences">
      {diff.create.map((t) => (
        <Row key={`c${t}`} name={t} kind="create" detail="only in the source" {...p} />
      ))}
      {diff.changed.map((d) => {
        const parts = [
          d.add.length && `+${d.add.length} column${d.add.length > 1 ? "s" : ""}`,
          d.alter.length && `${d.alter.length} changed`,
          d.drop.length && `${d.drop.length} only in the target`,
        ].filter(Boolean);
        return (
          <Row key={`a${d.table}`} name={d.table} kind="alter" detail={parts.join(" · ")} {...p} />
        );
      })}
      {diff.drop.map((t) => (
        <Row key={`d${t}`} name={t} kind="drop" detail="only in the target" {...p} />
      ))}
    </div>
  );
}
