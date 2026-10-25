// Sidebar and palette search (SCH-3): fuzzy (default) or contains, with highlight ranges.

export type MatchMode = "fuzzy" | "contains";

export interface Match {
  score: number;
  /** Indexes of matched characters, for highlighting. */
  indexes: number[];
}

export interface SearchResult<T> {
  item: T;
  match: Match;
}

const NO_QUERY: Match = { score: 0, indexes: [] };

function isWordStart(text: string, at: number): boolean {
  if (at === 0) return true;
  const before = text[at - 1] ?? "";
  const ch = text[at] ?? "";
  if (before === "_" || before === "." || before === " ") return true;
  // camelCase boundary
  return before >= "a" && before <= "z" && ch >= "A" && ch <= "Z";
}

/** Fuzzy subsequence match. Consecutive and word-start hits score higher. */
export function fuzzyMatch(query: string, text: string): Match | null {
  if (!query) return NO_QUERY;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  const indexes: number[] = [];
  let score = 0;
  let from = 0;
  let prev = -2;
  for (const ch of q) {
    const found = t.indexOf(ch, from);
    if (found < 0) return null;
    indexes.push(found);
    score += 1;
    if (found === prev + 1) score += 3;
    if (isWordStart(text, found)) score += 2;
    prev = found;
    from = found + 1;
  }
  score -= (t.length - q.length) * 0.01;
  return { score, indexes };
}

export function containsMatch(query: string, text: string): Match | null {
  if (!query) return NO_QUERY;
  const at = text.toLowerCase().indexOf(query.toLowerCase());
  if (at < 0) return null;
  const indexes = Array.from({ length: query.length }, (_, i) => at + i);
  return { score: at === 0 ? 2 : 1, indexes };
}

/** Filters and orders items by match; an empty query keeps the original order. */
export function search<T>(
  items: T[],
  query: string,
  mode: MatchMode,
  key: (t: T) => string,
): SearchResult<T>[] {
  const match = mode === "fuzzy" ? fuzzyMatch : containsMatch;
  const q = query.trim();
  const out: SearchResult<T>[] = [];
  for (const item of items) {
    const m = match(q, key(item));
    if (m) out.push({ item, match: m });
  }
  if (q) out.sort((a, b) => b.match.score - a.match.score);
  return out;
}
