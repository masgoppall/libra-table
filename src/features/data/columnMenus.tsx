// Column visibility (TablePlus "Select columns") and the column header's menu.

import { ArrowDown, ArrowUp, Check, Copy, EyeOff, Filter } from "lucide-react";
import { type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { copyText } from "@/lib/clipboard";
import { type SortSpec } from "@/types";

export interface ColumnMenuContext {
  column: string;
  hidden: string[];
  setHidden: (hidden: string[]) => void;
  setSort: (sort: SortSpec[]) => void;
  filterColumn: (column: string) => void;
}

export function headerMenu(c: ColumnMenuContext): MenuItem[] {
  return [
    {
      label: "Sort ascending",
      icon: <ArrowUp size={14} />,
      onClick: () => c.setSort([{ column: c.column, desc: false }]),
    },
    {
      label: "Sort descending",
      icon: <ArrowDown size={14} />,
      onClick: () => c.setSort([{ column: c.column, desc: true }]),
    },
    {
      label: "Filter by this column",
      icon: <Filter size={14} />,
      onClick: () => c.filterColumn(c.column),
    },
    SEPARATOR,
    { label: "Copy column name", icon: <Copy size={14} />, onClick: () => void copyText(c.column) },
    {
      label: "Hide column",
      icon: <EyeOff size={14} />,
      onClick: () => c.setHidden([...c.hidden, c.column]),
    },
    {
      label: "Show all columns",
      disabled: c.hidden.length === 0,
      onClick: () => c.setHidden([]),
    },
  ];
}

/** A checkbox item per column; the last visible column can't be hidden. */
export function visibilityMenu(
  columns: string[],
  hidden: string[],
  setHidden: (hidden: string[]) => void,
): MenuItem[] {
  const visible = columns.filter((n) => !hidden.includes(n));
  const toggle = (name: string) =>
    setHidden(hidden.includes(name) ? hidden.filter((n) => n !== name) : [...hidden, name]);
  return [
    { label: "Show all columns", disabled: hidden.length === 0, onClick: () => setHidden([]) },
    SEPARATOR,
    ...columns.map((name) => {
      const shown = !hidden.includes(name);
      return {
        label: name,
        icon: shown ? <Check size={14} /> : <span />,
        disabled: shown && visible.length === 1,
        onClick: () => toggle(name),
      };
    }),
  ];
}
