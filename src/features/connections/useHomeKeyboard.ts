// Arrow keys, Enter and Delete on the connection list.

import { type RefObject } from "react";
import { isEditingText } from "@/lib/actions";
import { useWindowKeys } from "@/hooks/useWindowKeys";
import { store } from "@/store";
import { type ConnectionView } from "@/types";
import { deleteConnection } from "./connectionActions";

export interface HomeKeyboardOptions {
  active: boolean;
  list: ConnectionView[];
  selected: string | null;
  searchRef: RefObject<HTMLInputElement>;
  onSelect: (id: string) => void;
  onConnect: (c: ConnectionView) => void;
  onClearSearch: () => void;
}

function scrollToRow(id: string) {
  document.querySelector(`[data-conn="${id}"]`)?.scrollIntoView({ block: "nearest" });
}

export function useHomeKeyboard(o: HomeKeyboardOptions) {
  useWindowKeys(o.active, (e) => {
    const { dialogs, settingsOpen } = store.get();
    if (e.defaultPrevented || dialogs.length || settingsOpen) return;
    const inSearch = document.activeElement === o.searchRef.current;
    if (isEditingText() && !inSearch) return;
    const i = o.list.findIndex((c) => c.id === o.selected);

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = e.key === "ArrowDown" ? Math.min(o.list.length - 1, i + 1) : Math.max(0, i - 1);
      const c = o.list[Math.max(0, next)];
      if (c) {
        o.onSelect(c.id);
        scrollToRow(c.id);
      }
    } else if (e.key === "Enter" && !e.metaKey) {
      const c = o.list[i] ?? (inSearch ? o.list[0] : undefined);
      if (c) {
        e.preventDefault();
        o.onConnect(c);
      }
    } else if ((e.key === "Backspace" || e.key === "Delete") && !inSearch && o.list[i]) {
      e.preventDefault();
      void deleteConnection(o.list[i]);
    } else if (e.key === "Escape" && inSearch) {
      o.onClearSearch();
      o.searchRef.current?.blur();
    }
  });
}
