/** Text with the matched characters of a search marked. */
export function Highlight({ text, indexes }: { text: string; indexes: number[] }) {
  if (!indexes.length) return <>{text}</>;
  const hit = new Set(indexes);
  return (
    <>
      {Array.from(text).map((ch, i) =>
        hit.has(i) ? <mark key={i}>{ch}</mark> : <span key={i}>{ch}</span>,
      )}
    </>
  );
}
