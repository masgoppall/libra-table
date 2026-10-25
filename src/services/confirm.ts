import { plural } from "@/lib/format";
import { store } from "@/store";

export interface DiscardWording {
  /** "Refreshing", "Reloading" */
  doing: string;
  /** "refresh", "reload" */
  verb: string;
  /** "uncommitted", "unsaved" */
  kind: string;
}

/** Asks before dropping `dirty` edits; true to go ahead (also when there are none). */
export async function confirmDiscard(dirty: number, w: DiscardWording): Promise<boolean> {
  if (!dirty) return true;
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Discard changes?",
    message: `${w.doing} drops ${plural(dirty, `${w.kind} change`)}.`,
    okLabel: `Discard & ${w.verb}`,
    danger: true,
  });
  return r.ok;
}
