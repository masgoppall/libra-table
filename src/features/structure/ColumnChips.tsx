/** Pick columns in order: click to add, click again to remove. */
export function ColumnChips(props: {
  names: string[];
  value: string[];
  onChange: (v: string[]) => void;
}) {
  const { names, value } = props;
  if (!names.length) return <span className="faint small">no columns</span>;
  const toggle = (n: string) =>
    props.onChange(value.includes(n) ? value.filter((v) => v !== n) : [...value, n]);
  return (
    <div className="chips">
      {names.map((n) => {
        const pos = value.indexOf(n);
        return (
          <button
            key={n}
            type="button"
            className={`chip${pos >= 0 ? " on" : ""}`}
            onClick={() => toggle(n)}
          >
            {pos >= 0 && value.length > 1 && <span className="chip-n">{pos + 1}</span>}
            {n}
          </button>
        );
      })}
    </div>
  );
}
