import { type SortSpec } from "@/types";

/** Header click cycles a column: ascending → descending → off. Shift keeps the others. */
export function nextSort(current: SortSpec[], column: string, additive: boolean): SortSpec[] {
  const existing = current.find((s) => s.column === column);
  if (!existing)
    return additive ? [...current, { column, desc: false }] : [{ column, desc: false }];
  if (!existing.desc) {
    const kept = additive ? current : [existing];
    return kept.map((s) => (s.column === column ? { ...s, desc: true } : s));
  }
  return additive ? current.filter((s) => s.column !== column) : [];
}
