// Pending edits with ⌘Z: every change pushes the previous state.

import { useEffect, useRef, useState } from "react";
import { store } from "@/store";
import { emptyPending, type Pending, pendingCount } from "./pending";

const UNDO_DEPTH = 100;

/** The edits of a tab, their count reported to the store (closing asks before losing them). */
export function useTabPendingEdits(tabId: string) {
  const edits = usePendingEdits();
  useEffect(() => {
    store.get().setPendingEdits(tabId, edits.dirty);
  }, [tabId, edits.dirty]);
  useEffect(() => () => store.get().setPendingEdits(tabId, 0), [tabId]);
  return edits;
}

/** Selected rows and the focused one. */
export function useRowSelection() {
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [focusRow, setFocusRow] = useState<number | null>(null);
  const selectRows = (next: Set<number>, focus: number | null) => {
    setSelected(next);
    setFocusRow(focus);
  };
  return { selected, setSelected, focusRow, selectRows };
}

export function usePendingEdits() {
  const [pending, setPendingState] = useState<Pending>(emptyPending);
  const [undoStack, setUndoStack] = useState<Pending[]>([]);
  // Several edits can land before a render (Enter then blur); always build on the latest.
  const ref = useRef(pending);

  const replace = (next: Pending) => {
    ref.current = next;
    setPendingState(next);
  };

  const update = (fn: (p: Pending) => Pending) => {
    const prev = ref.current;
    const next = fn(prev);
    if (next === prev) return;
    setUndoStack((s) => [...s.slice(-(UNDO_DEPTH - 1)), prev]);
    replace(next);
  };

  const reset = () => {
    replace(emptyPending());
    setUndoStack([]);
  };

  /** False when there is nothing to undo. */
  const undo = (): boolean => {
    const prev = undoStack[undoStack.length - 1];
    if (!prev) return false;
    setUndoStack((s) => s.slice(0, -1));
    replace(prev);
    return true;
  };

  const discard = () => update(() => emptyPending());

  return { pending, update, reset, undo, discard, dirty: pendingCount(pending) };
}

export type PendingEdits = ReturnType<typeof usePendingEdits>;
