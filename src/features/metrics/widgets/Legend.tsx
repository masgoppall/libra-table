// Series names with their color swatch; text stays in the text color.

export function Legend({ names }: { names: string[] }) {
  return (
    <div className="chart-legend">
      {names.map((n, i) => (
        <span key={`${n}-${i}`} className="chart-legend-item">
          <span className={`chart-swatch chart-s${i + 1}`} />
          {n}
        </span>
      ))}
    </div>
  );
}
