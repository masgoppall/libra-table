// Keys of the connection list, as in an outline view: ↑ / ↓ move through groups and
// connections, ← / → collapse and expand a group, Enter connects (or toggles a group),
// Delete removes the selected connection or group.

import { type RefObject } from "react";
import { isEditingText } from "@/lib/actions";
import { useWindowKeys } from "@/hooks/useWindowKeys";
import { store } from "@/store";
import { type ConnectionView } from "@/types";
import { deleteConnection, deleteGroup, toggleGroup } from "./connectionActions";
import { rowKey, type TreeRow } from "./welcome/tree";

export interface HomeKeyboardOptions {
  active: boolean;
  rows: TreeRow[];
  selected: string | null;
  searchRef: RefObject<HTMLInputElement>;
  onSelect: (key: string) => void;
  onConnect: (c: ConnectionView) => void;
  onClearSearch: () => void;
}

function scrollToRow(key: string) {
  document.querySelector(`[data-row="${key}"]`)?.scrollIntoView({ block: "nearest" });
}

export function useHomeKeyboard(o: HomeKeyboardOptions) {
  useWindowKeys(o.active, (e) => {
    const { dialogs, settingsOpen } = store.get();
    if (e.defaultPrevented || dialogs.length || settingsOpen) return;
    const inSearch = document.activeElement === o.searchRef.current;
    if (isEditingText() && !inSearch) return;
    const i = o.rows.findIndex((r) => rowKey(r) === o.selected);
    const row = o.rows[i];
    const select = (r: TreeRow | undefined) => {
      if (!r) return;
      o.onSelect(rowKey(r));
      scrollToRow(rowKey(r));
    };

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = e.key === "ArrowDown" ? Math.min(o.rows.length - 1, i + 1) : Math.max(0, i - 1);
      select(o.rows[Math.max(0, next)]);
    } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !inSearch && row) {
      e.preventDefault();
      const open = e.key === "ArrowRight";
      if (row.kind === "group" && row.collapsed === open) void toggleGroup(row.group);
      else if (!open) {
        // ← on a connection or an open group's child goes up to the group above it.
        const parent = o.rows
          .slice(0, i)
          .reverse()
          .find((r) => r.depth < row.depth);
        select(parent);
      }
    } else if (e.key === "Enter" && !e.metaKey) {
      const target = row ?? (inSearch ? o.rows.find((r) => r.kind === "conn") : undefined);
      if (!target) return;
      e.preventDefault();
      if (target.kind === "conn") o.onConnect(target.conn);
      else void toggleGroup(target.group);
    } else if ((e.key === "Backspace" || e.key === "Delete") && !inSearch && row) {
      e.preventDefault();
      void (row.kind === "conn" ? deleteConnection(row.conn) : deleteGroup(row.group));
    } else if (e.key === "Escape" && inSearch) {
      o.onClearSearch();
      o.searchRef.current?.blur();
    }
  });
}
