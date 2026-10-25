// Keyboard and menu actions of the active data tab.

import { isEditingText, LAYER, useActions } from "@/lib/actions";
import { toastInfo } from "@/services/errors";

export interface DataActionHandlers {
  dirty: number;
  canPrev: boolean;
  canNext: boolean;
  commit: () => void;
  reload: () => void;
  focusFilters: () => void;
  addRow: () => void;
  discard: () => void;
  undo: () => boolean;
  prevPage: () => void;
  nextPage: () => void;
  exportRows: () => void;
  openStructure: () => void;
  toggleDetail: () => void;
}

export function useDataActions(visible: boolean, h: DataActionHandlers) {
  useActions(LAYER.tab, visible, {
    save: () => (h.dirty ? h.commit() : toastInfo("Nothing to commit.")),
    refresh: h.reload,
    filter: h.focusFilters,
    "add-row": h.addRow,
    discard: () => (h.dirty ? h.discard() : false),
    // Text fields keep ⌘Z for themselves.
    undo: () => (isEditingText() ? false : h.undo()),
    "prev-page": () => (h.canPrev ? h.prevPage() : false),
    "next-page": () => (h.canNext ? h.nextPage() : false),
    export: h.exportRows,
    "open-structure": h.openStructure,
    "toggle-detail": h.toggleDetail,
  });
}
