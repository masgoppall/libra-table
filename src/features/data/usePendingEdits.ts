// Pending edits with ⌘Z: every change pushes the previous state.

import { useRef, useState } from "react";
import { emptyPending, type Pending, pendingCount } from "./pending";

const UNDO_DEPTH = 100;

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
